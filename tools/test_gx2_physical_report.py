import unittest

import gx2_physical_report as report


def guest(ms, message):
    seconds, milliseconds = divmod(ms, 1000)
    minutes, seconds = divmod(seconds, 60)
    return f"00;{minutes:02};{seconds:02};{milliseconds:03}: {message}"


def window(guest_ms, end_ms, span_ms, loops, paired, artwork_ms, scale=75, phase="gameplay"):
    return [
        guest(guest_ms, f"Horizon Cafe {kind}: t_ms={end_ms} {values}")
        for kind, values in (
            ("cadence", f"phase={phase} span_ms={span_ms} loops={loops} built={paired} paired={paired} partial=1"),
            ("pacing", "samples=9 median_ms=108 p95_ms=120 p99_ms=122 max_ms=130"),
            ("streaming", "ticks=Some(40) meshed=3 uploaded=2 upload_bytes=100 tv_sections=93 gamepad_sections=91"),
            ("wall game", f"game_ms=103 idle_ms=1 artwork_ms={artwork_ms} other_ms=1 invalid_steps=0"),
            ("wall render", "redraw_ms=13 submit_ms=22 gx2_wait_ms=26 outside_game_ms=Some(0.4)"),
            ("world", f"requested={scale} effective={scale} tv=1440x809 gamepad=641x360 surface_bytes=11378688 omitted_passes=0x8000"),
        )
    ]


class CafePhysicalReportTests(unittest.TestCase):
    def test_distinguishes_guest_trace_and_host_counter_clocks(self):
        span = 'Horizon startup "ph":"X","name":"server.sync_adopt","ts":1245000,"dur":881234'
        lines = [
            guest(8750, report.NIGHT),
            guest(9000, report.LOADING),
            guest(9250, report.ENTRY),
            guest(10100, span),
            span,
            guest(10800, report.SPRITES + 'hits=28639 misses=2 entries=1311 dirty=false'),
            guest(10810, report.IO + 'ui_atlas archive_opens=1 reads=705 disk_fallbacks=0'),
            guest(10900, 'Horizon: worldgen data loaded in 1.125s'),
            guest(13000, report.STARTUP + 'first original Game frame returned'),
            guest(13020, 'Horizon Cafe first original Game frame returned elapsed_ms=3691 world_ready=true'),
            *window(15100, 4000, 2000, 10, 10, 20),
            guest(15700, report.NATIVE + 'window_ms=2540 present_calls=10 hud_wait_ms=29'),
            guest(16000, report.MEMORY + 'bulk_free=234880920 bulk_largest=234880896'),
        ]
        result = report.read_session(lines)
        self.assertEqual(result['guest_elapsed_milestones_ms'], {
            'first original Game frame returned': 3750,
            'Night last observed': -500,
            'Night loading frame': -250,
            'first paired nonzero sampled terrain': 5850,
            'first sampled 75 sections per display': 5850,
        })
        self.assertEqual(result['world_ready_host_counter_ms'], 3691)
        self.assertEqual(result['generator_data_construction_ms'], 1125)
        self.assertEqual(result['startup_trace_spans']['server.sync_adopt'],
                         [{'trace_start_ms': 1245.0, 'duration_ms': 881.234}])
        self.assertEqual(result['cumulative_prepared_io']['ui_atlas']['reads'], 705)
        self.assertEqual(result['sprite_cache']['hits'], 28639)
        self.assertEqual(result['native_profile_windows'][0]['hud_wait_ms'], 29)
        self.assertEqual(result['memory_after_game_drop']['bulk_free'], 234880920)

    def test_summarizes_contiguous_windows_without_treating_serial_tails_as_pooled(self):
        lines = [guest(9250, report.ENTRY),
                 *window(13000, 4000, 2000, 10, 9, 42),
                 *window(15200, 6200, 2200, 20, 19, 15)]
        result = report.matched_windows(report.read_session(lines)['cafe_windows'], 4000, 6200)
        self.assertEqual(result['paired_per_second'], 6.667)
        self.assertEqual(result['elapsed_per_pair_ms'], 150.0)
        self.assertEqual(result['complete_submissions'], 26)
        self.assertEqual(result['partial_submissions'], 2)
        self.assertEqual(result['wall_ms_per_iteration']['artwork_ms'], 24)
        self.assertNotIn('guest_elapsed_ms', result['wall_ms_per_iteration'])
        self.assertEqual(result['median_of_window_pacing_ms']['p95_ms'], 120)
        self.assertNotIn('direct_hud_wait_ms_per_built_frame', result)
        lines += [guest(13000, 'Horizon Cafe CPU per frame: t_ms=4000 hud_wait_ms=40'),
                  guest(15200, 'Horizon Cafe CPU per frame: t_ms=6200 hud_wait_ms=30')]
        measured_leaf = report.matched_windows(report.read_session(lines)['cafe_windows'], 4000, 6200)
        self.assertEqual(measured_leaf['direct_hud_wait_ms_per_built_frame'], 33.214)

    def test_refuses_cross_session_and_unmatched_windows(self):
        with self.assertRaisesRegex(ValueError, 'one hosted entry'):
            report.read_session([guest(9250, report.ENTRY), guest(50000, report.ENTRY)])
        base = [guest(9250, report.ENTRY), *window(13000, 4000, 2000, 10, 9, 42)]
        for incompatible, message in (
            (window(15200, 6200, 2200, 20, 19, 15, scale=50), 'scale'),
            (window(15200, 6200, 2100, 20, 19, 15), 'contiguous'),
        ):
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                report.matched_windows(report.read_session(base + incompatible)['cafe_windows'], 4000, 6200)
        menu = [guest(9250, report.ENTRY), *window(13000, 4000, 2000, 10, 10, 42, phase='menu')]
        windows = report.read_session(menu)['cafe_windows']
        with self.assertRaisesRegex(ValueError, 'phase gameplay'):
            report.matched_windows(windows, 4000, 4000)
        self.assertEqual(report.matched_windows(windows, 4000, 4000, phase='menu')['paired_per_second'], 5.0)


if __name__ == '__main__':
    unittest.main()
