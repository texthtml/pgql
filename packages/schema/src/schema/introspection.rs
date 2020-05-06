#[derive(Clone, Debug)]
pub struct Relation {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Schema {
    pub name: String,
    pub relations: Vec<Relation>,
}

impl Schema {
    pub fn from(pool: &crate::connection::Pool, name: String) -> juniper::BoxFuture<Self> {
        Box::pin(async move {
            let query = "
                select table_name as name
                from information_schema.tables
                where table_schema = $1::text
            ";

            let relations = pool
                .get()
                .await
                .unwrap()
                .query(query, &[&name])
                .await
                .unwrap()
                .iter()
                .map(|table| Relation {
                    name: table.get("name"),
                })
                .collect();

            Self { name, relations }
        })
    }
}

#[derive(Clone, Debug)]
pub struct Database {
    name: String,
    pub schemas: Vec<Schema>,
}

impl Database {
    pub fn from(pool: &crate::connection::Pool) -> juniper::BoxFuture<Self> {
        Box::pin(async move {
            let name: String = pool
                .get()
                .await
                .unwrap()
                .query_one("select current_database()", &[])
                .await
                .unwrap()
                .get(0);

            Self {
                name: name.clone(),
                schemas: Self::find_schemas(pool, name).await,
            }
        })
    }

    fn find_schemas(
        pool: &crate::connection::Pool,
        database: String,
    ) -> juniper::BoxFuture<Vec<Schema>> {
        Box::pin(async move {
            let query = "
                select description
                from pg_shdescription
                join pg_database on objoid = pg_database.oid
                where datname = $1
            ";

            let comment: String = pool
                .get()
                .await
                .unwrap()
                .query_opt(query, &[&database])
                .await
                .unwrap()
                .map_or("public".into(), |row| row.get(0));

            futures::future::join_all(
                comment
                    .split(',')
                    .map(|name| Schema::from(pool, name.into())),
            )
            .await
        })
    }

    pub fn relations(self: &Self) -> Vec<Relation> {
        self.schemas
            .iter()
            .map(|schema| schema.relations.clone())
            .flatten()
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct Introspection {
    pub database: Database,
}

impl Introspection {
    pub fn from(pool: &crate::connection::Pool) -> juniper::BoxFuture<Self> {
        Box::pin(async move {
            Self {
                database: Database::from(pool).await,
            }
        })
    }

    pub fn relations(self: &Self) -> Vec<Relation> {
        self.database.relations()
    }
}
