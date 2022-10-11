use std::fs;

use syn::visit::{visit_file, Visit};

#[derive(Default, Debug)]
struct Stats {
    impl_trait_arguments: usize,
}

impl Visit<'_> for Stats {
    fn visit_type_impl_trait(&mut self, i: &syn::TypeImplTrait) {
        self.impl_trait_arguments += 1;
    }
}

fn main() {
    let source = fs::read_to_string("./src/main.rs").unwrap();
    let file = syn::parse_file(&source).unwrap();
    // dbg!(file);

    let mut stats = Stats::default();

    visit_file(&mut stats, &file);

    dbg!(stats);
}

// fn sum_numbers(nums: impl Iterator<Item = i32>) -> i32 {
//     nums.sum()
// }
