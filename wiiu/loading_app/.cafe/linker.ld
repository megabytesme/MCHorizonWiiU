OUTPUT_FORMAT("elf32-powerpc")
OUTPUT_ARCH(powerpc:common)

MEMORY {
   codemem (rx)  : ORIGIN = 0x02000000, LENGTH = 224M
   datamem (rw)  : ORIGIN = 0x10000000, LENGTH = 800M
   loadmem (rx)  : ORIGIN = 0xC0000000, LENGTH = 128M
}

ENTRY(__rpx_start)

SECTIONS
{
   /* Code Section - Single segment at 0x02000000 */
   . = ORIGIN(codemem);

   .syscall ALIGN(32) : {
      *(.syscall .syscall.*)
   } > codemem

   .text ALIGN(32) : {
      KEEP (*(.crt0))

      /* Text sections - prioritized by likelihood */
      *(.text.unlikely .text.*_unlikely .text.unlikely.*)
      *(.text.exit .text.exit.*)
      *(.text.startup .text.startup.*)
      *(.text.hot .text.hot.*)
      *(SORT(.text.sorted.*))
      *(.text .stub .text.* .gnu.linkonce.t.*)
      *(.glink)
      *(.gnu.warning)
   } > codemem

   /* Data Sections - Single segment at 0x10000000 */
   . = ORIGIN(datamem);

   .rodata ALIGN(8) : {
      /* PowerPC small data sections */
      *(.sdata2 .sdata2.* .gnu.linkonce.s2.*)
      *(.sbss2 .sbss2.* .gnu.linkonce.sb2.*)

      /* Read-only data */
      *(.rodata .rodata.* .gnu.linkonce.r.*)
      *(.rodata.str* .rodata.cst*)
      *(.rodata1)

      /* Exception handling tables */
      *(.gcc_except_table .gcc_except_table.*)
      *(.gnu_extab*)

      /* GOT and PLT for dynamic linking */
      *(.got2)
      *(.got)
      *(.plt)
      *(.branch_lt)
   } > datamem

   /* Exception frame information - same segment as .rodata */
   .eh_frame ALIGN(4) : {
      *(.eh_frame_hdr)
      *(.eh_frame_entry .eh_frame_entry.*)
      KEEP (*(.eh_frame))
      *(.eh_frame.*)
   } > datamem

   /* Initialized data - same segment */
   .data ALIGN(16) : {
      PROVIDE(__data_start = .);
      *(.data .data.* .gnu.linkonce.d.*)
      SORT(CONSTRUCTORS)
      *(.data1)
      *(.sdata .sdata.* .gnu.linkonce.s.*)
      PROVIDE(__data_end = .);
   } > datamem

   /* Uninitialized data - same segment */
   .bss ALIGN(16) (NOLOAD) : {
      *(.dynsbss)
      *(.sbss .sbss.* .gnu.linkonce.sb.*)
      *(.scommon)
      *(.dynbss)
      *(.bss .bss.* .gnu.linkonce.b.*)
      *(COMMON)
   } > datamem

   /* Wii U RPX Load Sections - Single segment at 0xC0000000 */
   . = ORIGIN(loadmem);

   .cafe_load_bounds : {
      LONG(0)
   } > loadmem

   .fexports ALIGN(32) : {
      KEEP( *(.fexports) )
   } > loadmem

   .dexports ALIGN(32) : {
      KEEP( *(.dexports) )
   } > loadmem

   /* Function imports from Wii U system libraries - all in same segment */
   .fimport_avm ALIGN(16) : {
      KEEP ( *(.fimport_avm) )
      *(.fimport_avm.*)
   } > loadmem

   .fimport_camera ALIGN(16) : {
      KEEP ( *(.fimport_camera) )
      *(.fimport_camera.*)
   } > loadmem

   .fimport_coreinit ALIGN(16) : {
      KEEP ( *(.fimport_coreinit) )
      *(.fimport_coreinit.*)
   } > loadmem

   .fimport_dc ALIGN(16) : {
      KEEP ( *(.fimport_dc) )
      *(.fimport_dc.*)
   } > loadmem

   .fimport_dmae ALIGN(16) : {
      KEEP ( *(.fimport_dmae) )
      *(.fimport_dmae.*)
   } > loadmem

   .fimport_drmapp ALIGN(16) : {
      KEEP ( *(.fimport_drmapp) )
      *(.fimport_drmapp.*)
   } > loadmem

   .fimport_erreula ALIGN(16) : {
      KEEP ( *(.fimport_erreula) )
      *(.fimport_erreula.*)
   } > loadmem

   .fimport_gx2 ALIGN(16) : {
      KEEP ( *(.fimport_gx2) )
      *(.fimport_gx2.*)
   } > loadmem

   .fimport_h264 ALIGN(16) : {
      KEEP ( *(.fimport_h264) )
      *(.fimport_h264.*)
   } > loadmem

   .fimport_lzma920 ALIGN(16) : {
      KEEP ( *(.fimport_lzma920) )
      *(.fimport_lzma920.*)
   } > loadmem

   .fimport_mic ALIGN(16) : {
      KEEP ( *(.fimport_mic) )
      *(.fimport_mic.*)
   } > loadmem

   .fimport_nfc ALIGN(16) : {
      KEEP ( *(.fimport_nfc) )
      *(.fimport_nfc.*)
   } > loadmem

   .fimport_nio_prof ALIGN(16) : {
      KEEP ( *(.fimport_nio_prof) )
      *(.fimport_nio_prof.*)
   } > loadmem

   .fimport_nlibcurl ALIGN(16) : {
      KEEP ( *(.fimport_nlibcurl) )
      *(.fimport_nlibcurl.*)
   } > loadmem

   .fimport_nlibnss2 ALIGN(16) : {
      KEEP ( *(.fimport_nlibnss2) )
      *(.fimport_nlibnss2.*)
   } > loadmem

   .fimport_nlibnss ALIGN(16) : {
      KEEP ( *(.fimport_nlibnss) )
      *(.fimport_nlibnss.*)
   } > loadmem

   .fimport_nn_ac ALIGN(16) : {
      KEEP ( *(.fimport_nn_ac) )
      *(.fimport_nn_ac.*)
   } > loadmem

   .fimport_nn_acp ALIGN(16) : {
      KEEP ( *(.fimport_nn_acp) )
      *(.fimport_nn_acp.*)
   } > loadmem

   .fimport_nn_act ALIGN(16) : {
      KEEP ( *(.fimport_nn_act) )
      *(.fimport_nn_act.*)
   } > loadmem

   .fimport_nn_aoc ALIGN(16) : {
      KEEP ( *(.fimport_nn_aoc) )
      *(.fimport_nn_aoc.*)
   } > loadmem

   .fimport_nn_boss ALIGN(16) : {
      KEEP ( *(.fimport_nn_boss) )
      *(.fimport_nn_boss.*)
   } > loadmem

   .fimport_nn_ccr ALIGN(16) : {
      KEEP ( *(.fimport_nn_ccr) )
      *(.fimport_nn_ccr.*)
   } > loadmem

   .fimport_nn_cmpt ALIGN(16) : {
      KEEP ( *(.fimport_nn_cmpt) )
      *(.fimport_nn_cmpt.*)
   } > loadmem

   .fimport_nn_dlp ALIGN(16) : {
      KEEP ( *(.fimport_nn_dlp) )
      *(.fimport_nn_dlp.*)
   } > loadmem

   .fimport_nn_ec ALIGN(16) : {
      KEEP ( *(.fimport_nn_ec) )
      *(.fimport_nn_ec.*)
   } > loadmem

   .fimport_nn_fp ALIGN(16) : {
      KEEP ( *(.fimport_nn_fp) )
      *(.fimport_nn_fp.*)
   } > loadmem

   .fimport_nn_hai ALIGN(16) : {
      KEEP ( *(.fimport_nn_hai) )
      *(.fimport_nn_hai.*)
   } > loadmem

   .fimport_nn_hpad ALIGN(16) : {
      KEEP ( *(.fimport_nn_hpad) )
      *(.fimport_nn_hpad.*)
   } > loadmem

   .fimport_nn_idbe ALIGN(16) : {
      KEEP ( *(.fimport_nn_idbe) )
      *(.fimport_nn_idbe.*)
   } > loadmem

   .fimport_nn_ndm ALIGN(16) : {
      KEEP ( *(.fimport_nn_ndm) )
      *(.fimport_nn_ndm.*)
   } > loadmem

   .fimport_nn_nets2 ALIGN(16) : {
      KEEP ( *(.fimport_nn_nets2) )
      *(.fimport_nn_nets2.*)
   } > loadmem

   .fimport_nn_nfp ALIGN(16) : {
      KEEP ( *(.fimport_nn_nfp) )
      *(.fimport_nn_nfp.*)
   } > loadmem

   .fimport_nn_nim ALIGN(16) : {
      KEEP ( *(.fimport_nn_nim) )
      *(.fimport_nn_nim.*)
   } > loadmem

   .fimport_nn_olv ALIGN(16) : {
      KEEP ( *(.fimport_nn_olv) )
      *(.fimport_nn_olv.*)
   } > loadmem

   .fimport_nn_pdm ALIGN(16) : {
      KEEP ( *(.fimport_nn_pdm) )
      *(.fimport_nn_pdm.*)
   } > loadmem

   .fimport_nn_save ALIGN(16) : {
      KEEP ( *(.fimport_nn_save) )
      *(.fimport_nn_save.*)
   } > loadmem

   .fimport_nn_sl ALIGN(16) : {
      KEEP ( *(.fimport_nn_sl) )
      *(.fimport_nn_sl.*)
   } > loadmem

   .fimport_nn_spm ALIGN(16) : {
      KEEP ( *(.fimport_nn_spm) )
      *(.fimport_nn_spm.*)
   } > loadmem

   .fimport_nn_temp ALIGN(16) : {
      KEEP ( *(.fimport_nn_temp) )
      *(.fimport_nn_temp.*)
   } > loadmem

   .fimport_nn_uds ALIGN(16) : {
      KEEP ( *(.fimport_nn_uds) )
      *(.fimport_nn_uds.*)
   } > loadmem

   .fimport_nn_vctl ALIGN(16) : {
      KEEP ( *(.fimport_nn_vctl) )
      *(.fimport_nn_vctl.*)
   } > loadmem

   .fimport_nsysccr ALIGN(16) : {
      KEEP ( *(.fimport_nsysccr) )
      *(.fimport_nsysccr.*)
   } > loadmem

   .fimport_nsyshid ALIGN(16) : {
      KEEP ( *(.fimport_nsyshid) )
      *(.fimport_nsyshid.*)
   } > loadmem

   .fimport_nsyskbd ALIGN(16) : {
      KEEP ( *(.fimport_nsyskbd) )
      *(.fimport_nsyskbd.*)
   } > loadmem

   .fimport_nsysnet ALIGN(16) : {
      KEEP ( *(.fimport_nsysnet) )
      *(.fimport_nsysnet.*)
   } > loadmem

   .fimport_nsysuhs ALIGN(16) : {
      KEEP ( *(.fimport_nsysuhs) )
      *(.fimport_nsysuhs.*)
   } > loadmem

   .fimport_nsysuvd ALIGN(16) : {
      KEEP ( *(.fimport_nsysuvd) )
      *(.fimport_nsysuvd.*)
   } > loadmem

   .fimport_ntag ALIGN(16) : {
      KEEP ( *(.fimport_ntag) )
      *(.fimport_ntag.*)
   } > loadmem

   .fimport_padscore ALIGN(16) : {
      KEEP ( *(.fimport_padscore) )
      *(.fimport_padscore.*)
   } > loadmem

   .fimport_proc_ui ALIGN(16) : {
      KEEP ( *(.fimport_proc_ui) )
      *(.fimport_proc_ui.*)
   } > loadmem

   .fimport_sndcore2 ALIGN(16) : {
      KEEP ( *(.fimport_sndcore2) )
      *(.fimport_sndcore2.*)
   } > loadmem

   .fimport_snd_core ALIGN(16) : {
      KEEP ( *(.fimport_snd_core) )
      *(.fimport_snd_core.*)
   } > loadmem

   .fimport_snduser2 ALIGN(16) : {
      KEEP ( *(.fimport_snduser2) )
      *(.fimport_snduser2.*)
   } > loadmem

   .fimport_snd_user ALIGN(16) : {
      KEEP ( *(.fimport_snd_user) )
      *(.fimport_snd_user.*)
   } > loadmem

   .fimport_swkbd ALIGN(16) : {
      KEEP ( *(.fimport_swkbd) )
      *(.fimport_swkbd.*)
   } > loadmem

   .fimport_sysapp ALIGN(16) : {
      KEEP ( *(.fimport_sysapp) )
      *(.fimport_sysapp.*)
   } > loadmem

   .fimport_tcl ALIGN(16) : {
      KEEP ( *(.fimport_tcl) )
      *(.fimport_tcl.*)
   } > loadmem

   .fimport_tve ALIGN(16) : {
      KEEP ( *(.fimport_tve) )
      *(.fimport_tve.*)
   } > loadmem

   .fimport_uac ALIGN(16) : {
      KEEP ( *(.fimport_uac) )
      *(.fimport_uac.*)
   } > loadmem

   .fimport_uac_rpl ALIGN(16) : {
      KEEP ( *(.fimport_uac_rpl) )
      *(.fimport_uac_rpl.*)
   } > loadmem

   .fimport_usb_mic ALIGN(16) : {
      KEEP ( *(.fimport_usb_mic) )
      *(.fimport_usb_mic.*)
   } > loadmem

   .fimport_uvc ALIGN(16) : {
      KEEP ( *(.fimport_uvc) )
      *(.fimport_uvc.*)
   } > loadmem

   .fimport_uvd ALIGN(16) : {
      KEEP ( *(.fimport_uvd) )
      *(.fimport_uvd.*)
   } > loadmem

   .fimport_vpadbase ALIGN(16) : {
      KEEP ( *(.fimport_vpadbase) )
      *(.fimport_vpadbase.*)
   } > loadmem

   .fimport_vpad ALIGN(16) : {
      KEEP ( *(.fimport_vpad) )
      *(.fimport_vpad.*)
   } > loadmem

   /* Data imports from Wii U system libraries - same segment */
   .dimport_coreinit ALIGN(16) : {
      KEEP ( *(.dimport_coreinit) )
      *(.dimport_coreinit.*)
   } > loadmem

   .dimport_nn_act ALIGN(16) : {
      KEEP ( *(.dimport_nn_act) )
      *(.dimport_nn_act.*)
   } > loadmem

   .dimport_nn_boss ALIGN(16) : {
      KEEP ( *(.dimport_nn_boss) )
      *(.dimport_nn_boss.*)
   } > loadmem

   .dimport_nn_ec ALIGN(16) : {
      KEEP ( *(.dimport_nn_ec) )
      *(.dimport_nn_ec.*)
   } > loadmem

   .dimport_nn_nim ALIGN(16) : {
      KEEP ( *(.dimport_nn_nim) )
      *(.dimport_nn_nim.*)
   } > loadmem

   .dimport_nn_sl ALIGN(16) : {
      KEEP ( *(.dimport_nn_sl) )
      *(.dimport_nn_sl.*)
   } > loadmem

   .dimport_nn_uds ALIGN(16) : {
      KEEP ( *(.dimport_nn_uds) )
      *(.dimport_nn_uds.*)
   } > loadmem

   /* Discard debugging and metadata sections */
   /DISCARD/ : {
      *(.abs)
      *(.comment)
      *(.gnu.attributes)
      *(.gnu.version)
      *(.gnu.version_d)
      *(.gnu.version_r)
   }
}
