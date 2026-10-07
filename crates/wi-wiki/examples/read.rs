//! Reads a saved page and prints what the Wiki tab would show, as text:
//! `cargo run -p wi-wiki --example read -- page.html "Title" [json]`
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let html = std::fs::read_to_string(&args[1]).expect("a saved page");
    let started = std::time::Instant::now();
    let article = wi_wiki::parse_article(&html, &args[2]);
    let took = started.elapsed();
    if args.get(3).map(String::as_str) == Some("json") {
        println!("{}", serde_json::to_string_pretty(&article).unwrap());
        return;
    }
    println!("{}", wi_wiki::plain_text(&article, 1_000_000));
    eprintln!(
        "{} blocks, {} sections, {} references, {} bytes of JSON, read in {took:?}",
        article["blocks"].as_array().map_or(0, Vec::len),
        article["sections"].as_array().map_or(0, Vec::len),
        article["refs"].as_array().map_or(0, Vec::len),
        article.to_string().len(),
    );
}
