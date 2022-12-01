unsafe fn foo() {
    let i = std::mem::transmute::<[u8; 4], u32>(raw_bytes);
}
