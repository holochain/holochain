use hdk::prelude::*;
use integrity::{BookEntry, EntryTypes, LinkTypes};

mod integrity;

#[hdk_extern]
fn add_book_entry(author_and_name: (String, String)) -> ExternResult<()> {
    // Use path-sharding to split author's name into single character paths.
    let path_string = format!(
        "1:{}#{}",
        author_and_name.0.len(),
        author_and_name
            .0
            .to_lowercase()
            .replace(char::is_whitespace, "-"),
    );
    let path = Path::from(path_string).typed(LinkTypes::AuthorPath)?;

    let book_tag: LinkTag = author_and_name.1.clone().into();

    if !get_links(
        LinkQuery::new(
            path.path_entry_hash()?,
            LinkTypes::AuthorBook.try_into_filter()?,
        )
        .tag_prefix(book_tag.clone()),
        GetStrategy::default(),
    )?
    .is_empty()
    {
        // Link to book with name as tag exists so the book should exist.
        return Ok(());
    }

    let book_action_hash = create_entry(EntryTypes::BookEntry(BookEntry {
        name: author_and_name.1,
    }))?;
    let book_action = must_get_action(book_action_hash)?;
    let book_entry_hash = book_action
        .action()
        .entry_hash()
        .expect("created book action has no entry hash");

    // Create links for each component in the path.
    path.ensure()?;

    // Link the end of the path to the book itself with the book's name as the tag.
    create_link(
        path.path_entry_hash()?,
        book_entry_hash.clone(),
        LinkTypes::AuthorBook,
        book_tag,
    )?;

    Ok(())
}

fn recursively_find_books(path: TypedPath) -> ExternResult<Vec<Link>> {
    let mut links = Vec::new();

    for child in path.children_paths()? {
        let child_links = get_links(
            LinkQuery::new(
                child.path_entry_hash()?,
                LinkTypes::AuthorBook.try_into_filter()?,
            ),
            GetStrategy::default(),
        )?;
        links.extend(child_links);
        links.extend(recursively_find_books(child)?);
    }

    Ok(links)
}

#[hdk_extern]
fn find_books_from_author(author: String) -> ExternResult<Vec<BookEntry>> {
    let path_string = format!("1:{}#{}", author.len(), author.to_lowercase(),);
    let path = Path::from(path_string).typed(LinkTypes::AuthorPath)?;

    // Path-sharding appends an extra leaf to the path so remove it.
    // Example: if trying to find authors beginning with 'ab' then the path will be "a.b.ab" which will
    // find nothing so instead, use the parent path which is "a.b".
    let path = path
        .parent()
        .ok_or(wasm_error!("Could not get path from author"))?;

    // Because we start at the parent path then we never need to check ourselves and instead only
    // our children because we are our parent's child.
    // Example 1: Given a full author's name "bob", the path will be "b.o.b.bob" so take our parent
    // "b.o.b" and then search its children and we find "b.o.b.bob" which is where the links are.
    // Example 2: Given a part of an author's name "bo", the path will be "b.o.bo" which is not
    // valid so we take the parent "b.o" and recursively search its children to find "b.o.b.bob".
    let book_links = recursively_find_books(path)?;

    let books = book_links
        .into_iter()
        .filter_map(|link| link.target.into_entry_hash())
        .filter_map(|entry_hash| must_get_entry(entry_hash).map(|entry| entry.content).ok())
        .filter_map(|entry_content| BookEntry::try_from(entry_content).ok())
        .collect();

    Ok(books)
}
