use std::fs::create_dir_all;
use holochain_sqlite::db::{DbKindWasm, DbWrite};
use holochain_sqlite::error::DatabaseResult;

#[tokio::test(flavor = "multi_thread")]
async fn open_long_path() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    // 280 characters, split into segments
    let path = tmp_dir.path().join("alskdjflaskdjflaskdjflaskdjfals/kdfjalsdkfjlasdkfjlasdfjalsdkfjasldkfjasldfkjalsdkfjalsdkfjasldkfjksaldfjslakdfjlskdfjlkskdjflasdj/fklasdfjklasdfsaldfkjlsdkfjalsdfjaklsdfjalskfjdlaskdjflsakjfklsadjlask/dflasjdfklsjdfklsdjfklasdjflsakdjlasdjfklsajdflkasdjflsakjfdlsdkjflskdfjlksdjfaaa");
    create_dir_all(&path).unwrap();

    let db = DbWrite::open(&path, DbKindWasm).unwrap();

    db.write_async(|txn| -> DatabaseResult<()> {
        txn.execute("CREATE TABLE test (name TEXT)", [])?;
        txn.execute("INSERT INTO test (name) VALUES ('hello')", [])?;
        Ok(())
    }).await.unwrap();
}
