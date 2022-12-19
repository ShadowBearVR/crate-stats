// rust-lang/rust-analyzer/crates/hir-def/src/attr.rs:958
// ======================================================

fn collect_attrs(
    owner: &dyn ast::HasAttrs,
) -> impl Iterator<Item = (AttrId, Either<ast::Attr, ast::Comment>)> {
    let inner_attrs = inner_attributes(owner.syntax()).into_iter().flatten();
    let outer_attrs =
        ast::AttrDocCommentIter::from_syntax_node(owner.syntax()).filter(|el| match el {
            Either::Left(attr) => attr.kind().is_outer(),
            Either::Right(comment) => comment.is_outer(),
        });
    outer_attrs
        .chain(inner_attrs)
        .enumerate()
        .map(|(id, attr)| (AttrId { ast_index: id as u32 }, attr))
}

gen fn collect_attrs(
    owner: &dyn ast::HasAttrs,
) -> (AttrId, Either<ast::Attr, ast::Comment>) {
    let mut ast_index = 0;
    for attr in ast::AttrDocCommentIter::from_syntax_node(owner.syntax()) {
        if match attr {
            Either::Left(attr) => attr.kind().is_outer(),
            Either::Right(comment) => comment.is_outer(),
        } {
            yield (AttrId { ast_index }, attr);
            ast_index += 1;
        }
    }

    for inner_attrs in inner_attributes(owner.syntax()) {
        for attr in inner_attrs {
            yield (AttrId { ast_index }, attr);
            ast_index += 1;
        }
    }
}

// racer/rust-racer/src/racer/nameres.rs:2297
// ==========================================

fn search_for_trait_items<'s, 'sess: 's>(
    traitm: Match,
    search_str: &'s str,
    search_type: SearchType,
    includes_assoc_fn: bool,
    includes_assoc_ty_and_const: bool,
    session: &'sess Session<'sess>,
) -> impl 's + Iterator<Item = Match> {
    let traits = collect_inherited_traits(traitm, session);
    traits
        .into_iter()
        .filter_map(move |tr| {
            let src = session.load_source_file(&tr.filepath);
            src[tr.point.0..].find('{').map(|start| {
                search_scope_for_methods(
                    tr.point + BytePos(start + 1),
                    src.as_src(),
                    search_str,
                    &tr.filepath,
                    includes_assoc_fn,
                    includes_assoc_ty_and_const,
                    search_type,
                    session,
                )
            })
        })
        .flatten()
}

gen fn search_for_trait_items<'s, 'sess: 's>(
    traitm: Match,
    search_str: &'s str,
    search_type: SearchType,
    includes_assoc_fn: bool,
    includes_assoc_ty_and_const: bool,
    session: &'sess Session<'sess>,
) -> Match {
    for tr in collect_inherited_traits(traitm, session) {
        let src = session.load_source_file(&tr.filepath);
        if let Some(start) = src[tr.point.0..].find('{') {
            for m in search_scope_for_methods(
                tr.point + BytePos(start + 1),
                src.as_src(),
                search_str,
                &tr.filepath,
                includes_assoc_fn,
                includes_assoc_ty_and_const,
                search_type,
                session,
            ) {
                yield m;
            }
        }
    }
}

// rust-lang/rust-analyzer/crates/ide-db/src/items_locator.rs:105
// ==============================================================

fn find_items<'a>(
    sema: &'a Semantics<'_, RootDatabase>,
    krate: Crate,
    assoc_item_search: AssocItemSearch,
    local_query: symbol_index::Query,
    external_query: import_map::Query,
) -> impl Iterator<Item = ItemInNs> + 'a {
    let _p = profile::span("find_items");
    let db = sema.db;

    let external_importables =
        krate.query_external_importables(db, external_query).map(|external_importable| {
            match external_importable {
                Either::Left(module_def) => ItemInNs::from(module_def),
                Either::Right(macro_def) => ItemInNs::from(macro_def),
            }
        });

    // Query the local crate using the symbol index.
    let local_results = symbol_index::crate_symbols(db, krate, local_query)
        .into_iter()
        .filter_map(move |local_candidate| get_name_definition(sema, &local_candidate))
        .filter_map(|name_definition_to_import| match name_definition_to_import {
            Definition::Macro(macro_def) => Some(ItemInNs::from(macro_def)),
            def => <Option<_>>::from(def),
        });

    external_importables.chain(local_results).filter(move |&item| match assoc_item_search {
        AssocItemSearch::Include => true,
        AssocItemSearch::Exclude => !is_assoc_item(item, sema.db),
        AssocItemSearch::AssocItemsOnly => is_assoc_item(item, sema.db),
    })
}

gen fn find_items<'a>(
    sema: &'a Semantics<'_, RootDatabase>,
    krate: Crate,
    assoc_item_search: AssocItemSearch,
    local_query: symbol_index::Query,
    external_query: import_map::Query,
) -> ItemInNs {
    let _p = profile::span("find_items");
    let db = sema.db;
    let is_item = |&item| match assoc_item_search {
        AssocItemSearch::Include => true,
        AssocItemSearch::Exclude => !is_assoc_item(item, sema.db),
        AssocItemSearch::AssocItemsOnly => is_assoc_item(item, sema.db),
    };

    for external_importable in krate.query_external_importables(db, external_query) {
        let item = match external_importable {
            Either::Left(module_def) => ItemInNs::from(module_def),
            Either::Right(macro_def) => ItemInNs::from(macro_def),
        };
        if is_item(&item) {
            yield item;
        }
    }

    // Query the local crate using the symbol index.
    for local_canidate in symbol_index::crate_symbols(db, krate, local_query) {
        if let Some(name_definition_to_import) = get_name_definition(sema, &local_candidate) {
            match name_definition_to_import {
                Definition::Macro(macro_def) => {
                    let item: ItemInNs = macro_def.into();
                    if is_item(&item) {
                        yeild item;
                    }
                },
                def => if let Some(item) = def.into() {
                    if is_item(&item) {
                        yeild item;
                    }
                },
            };
        }
    }
}

// enso-org/enso/lib/rust/ensogl/component/grid-view/src/visible_area.rs:102
// =========================================================================

fn all_visible_locations(
    v: Viewport,
    entry_size: Vector2,
    row_count: usize,
    col_count: usize,
    column_widths: &ColumnWidths,
) -> impl Iterator<Item = (Row, Col)> {
    let visible_rows = visible_rows(v, entry_size, row_count);
    let visible_cols = visible_columns(v, entry_size, col_count, column_widths);
    itertools::iproduct!(visible_rows, visible_cols)
}

gen fn all_visible_locations(
    v: Viewport,
    entry_size: Vector2,
    row_count: usize,
    col_count: usize,
    column_widths: &ColumnWidths,
) -> (Row, Col) {
    for row in visible_rows(v, entry_size, row_count) {
        for col in visible_columns(v, entry_size, col_count, column_widths) {
            yield (row, col);
        }
    }
}

// spearow/juice/juice-examples/mackey-glass-rnn-regression/src/main.rs:61
// =======================================================================

fn data_generator(data: DataMode) -> impl Iterator<Item = (f32, Vec<f32>)> {
    let file = File::open(data.as_path()).expect("File opens as read. qed");
    let rdr = csv::ReaderBuilder::new()
        .delimiter(b',')
        .trim(csv::Trim::All)
        .from_reader(file);

    assert!(rdr.has_headers());

    rdr.into_deserialize()
        .enumerate()
        .map(move |(idx, row): (_, Result<Record, _>)| {
            let record: Record = match row {
                Ok(record) => record,
                Err(err) => panic!(
                    "All rows (including row {} (base-0)) in assets are valid. qed -> {:?}",
                    idx, err
                ),
            };
            (record.target(), record.bs())
        })
}

gen fn data_generator(data: DataMode) -> (f32, Vec<f32>) {
    let file = File::open(data.as_path()).expect("File opens as read. qed");
    let rdr = csv::ReaderBuilder::new()
        .delimiter(b',')
        .trim(csv::Trim::All)
        .from_reader(file);

    assert!(rdr.has_headers());

    for (idx, row) in rdr.into_deserialize() {
        let record: Record = match row {
            Ok(record) => record,
            Err(err) => panic!(
                "All rows (including row {} (base-0)) in assets are valid. qed -> {:?}",
                idx, err
            ),
        };
        yield (record.target(), record.bs());
    }
}
