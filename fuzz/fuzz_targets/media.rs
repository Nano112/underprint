#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = underprint::load_image(data, &underprint::ImagePolicy::default());
});
