use once_cell::sync::Lazy;

static MIGRATIONS: Lazy<Schema> = Lazy::new(|| {
    let migration_0 = Migration::initial(include!("schema/initial.sql"));

    Schema {
        current_index: 0,
        migrations: vec![migration_0],
    }
});

pub struct Schema {
    current_index: u16,
    migrations: Vec<Migration>,
}

pub struct Migration {
    schema: Sql,
    forward: Sql,
    backward: Option<Sql>,
}

impl Migration {
    pub fn initial(schema: &str) -> Self {
        Self {
            schema: schema.into(),
            forward: "".into(),
            backward: None,
        }
    }
}

type Sql = String;
