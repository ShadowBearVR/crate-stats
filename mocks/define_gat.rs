struct Mock;

trait LendingIterator {
    type Item<'a>;

    fn next(&'a self) -> Self::Item<'a>;
}
