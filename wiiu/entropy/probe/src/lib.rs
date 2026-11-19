//! Opt-in executable probe for the real Cafe entropy adapter.

#[cfg(target_os = "cafe")]
#[no_mangle]
pub extern "C" fn horizon_entropy_probe() -> u32 {
    horizon_wiiu_entropy::link();
    let mut failures = 0;
    let mut a = [0; 256];
    let mut b = [0; 256];
    let mut c = [0x55; 256];
    if getrandom02::getrandom(&mut a).is_err() {
        failures |= 1;
    }
    if getrandom03::fill(&mut b).is_err() {
        failures |= 2;
    }
    if getrandom04::fill(&mut c).is_err() {
        failures |= 4;
    }
    if a.iter().all(|&x| x == 0) {
        failures |= 8;
    }
    if b.iter().all(|&x| x == 0) {
        failures |= 16;
    }
    if c.iter().all(|&x| x == 0) || c.iter().all(|&x| x == 0x55) {
        failures |= 32;
    }
    if a == b || b == c || a == c {
        failures |= 64;
    }
    let mut uninit03 = [std::mem::MaybeUninit::<u8>::uninit(); 64];
    let mut uninit04 = [std::mem::MaybeUninit::<u8>::uninit(); 64];
    if getrandom03::fill_uninit(&mut uninit03).map_or(true, |v| v.iter().all(|&x| x == 0)) {
        failures |= 128;
    }
    if getrandom04::fill_uninit(&mut uninit04).map_or(true, |v| v.iter().all(|&x| x == 0)) {
        failures |= 256;
    }
    failures
}
