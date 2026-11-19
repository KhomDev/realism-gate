#![no_main]

use libfuzzer_sys::fuzz_target;

// The property: the typescript frontend never panics on any input.
fuzz_target!(|data: &[u8]| {
    realism_gate_fuzz::typescript(data);
});
