unsafe fn foo() {
    let i: u32 = std::mem::transmute(raw_bytes);
}
