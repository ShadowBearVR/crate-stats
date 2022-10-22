trait Mock<A, B, C> {
    type AT;
}

fn stuff(_: impl Mock<i32, u32, f32>) {}
