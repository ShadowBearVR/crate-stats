async fn foo() {
    async {
        async {
            let a = 0;
        };
    };
    async {
        let b = 1;
    };
    async {
        let c = 2;
    };
}
