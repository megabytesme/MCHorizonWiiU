import unittest

from gx2_cemu_acceptance import MIN_MOVEMENT_BLOCKS, movement_along_input


class MovementEvidenceTests(unittest.TestCase):
    def test_displacement_in_requested_world_direction(self):
        start = (-13.0, 68.0, 96.0)
        cases = [
            ("backward", 0.0, (-13.0, 68.0, 94.0)),
            ("forward", -90.0, (-11.0, 68.0, 96.0)),
            ("left", 90.0, (-13.0, 68.0, 98.0)),
            ("right", 0.0, (-15.0, 68.0, 96.0)),
        ]
        for direction, yaw, end in cases:
            with self.subTest(direction=direction, yaw=yaw):
                self.assertAlmostEqual(movement_along_input(start, end, yaw, direction), 2.0)

    def test_other_movement_does_not_verify_backward_input(self):
        start = (0.0, 68.0, 96.0)
        for end in ((0.0, 68.0, 97.0), (1.0, 68.0, 96.0), (0.0, 69.0, 96.0),
                    (0.0, 68.0, 96.0 - 1e-6)):
            with self.subTest(end=end):
                self.assertLess(movement_along_input(start, end, 0.0, "backward"), MIN_MOVEMENT_BLOCKS)


if __name__ == "__main__":
    unittest.main()
