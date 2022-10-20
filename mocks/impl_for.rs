struct Mock;

impl Iterator for Mock {
    type Item = ();

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
