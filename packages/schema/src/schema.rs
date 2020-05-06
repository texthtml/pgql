mod introspection;

use itertools::Itertools;
use std::collections::HashMap;

impl juniper::Context for crate::context::Context {}

type ReturnType<'a> = juniper::BoxFuture<'a, juniper::ExecutionResult<juniper::DefaultScalarValue>>;
type Resolver = for<'a> fn(
    &'a juniper::Executor<crate::context::Context, juniper::DefaultScalarValue>,
) -> ReturnType<'a>;

trait Registrable: Send + Sync + std::fmt::Debug {
    fn register<'r>(
        self: &Self,
        registry: &mut juniper::Registry<'r>,
    ) -> juniper::meta::Field<'r, juniper::DefaultScalarValue>;
    fn name(self: &Self) -> String;
    fn resolve<'a>(
        self: &Self,
        executor: &'a juniper::Executor<crate::context::Context, juniper::DefaultScalarValue>,
    ) -> ReturnType<'a>;
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct FieldInfo<S> {
    resolve_type: std::marker::PhantomData<S>,
    name: String,
    #[derivative(Debug = "ignore")]
    resolver: Resolver,
}

impl<S> Registrable for FieldInfo<S>
where
    S: juniper::GraphQLType<TypeInfo = ()> + Send + Sync + std::fmt::Debug,
{
    fn name(self: &Self) -> String {
        self.name.to_owned()
    }

    fn register<'r>(
        self: &Self,
        registry: &mut juniper::Registry<'r>,
    ) -> juniper::meta::Field<'r, juniper::DefaultScalarValue> {
        registry.field::<S>(&self.name, &())
    }

    fn resolve<'a>(
        self: &Self,
        executor: &'a juniper::Executor<crate::context::Context, juniper::DefaultScalarValue>,
    ) -> ReturnType<'a> {
        (self.resolver)(executor)
    }
}

#[derive(Debug)]
pub struct TypeInfo {
    name: String,
    fields: HashMap<String, Box<dyn Registrable>>,
}

impl TypeInfo {
    fn new(name: String, fields: Vec<Box<dyn Registrable>>) -> Self {
        TypeInfo {
            name,
            fields: {
                let mut fields_builder = HashMap::new();

                for field in fields {
                    fields_builder.insert(field.name().to_owned(), field);
                }

                fields_builder
            },
        }
    }
}

#[derive(Debug)]
pub struct Query {}

impl juniper::GraphQLTypeAsync<juniper::DefaultScalarValue> for Query {
    fn resolve_field_async<'a>(
        &'a self,
        info: &'a Self::TypeInfo,
        field_name: &'a str,
        _arguments: &'a juniper::Arguments<juniper::DefaultScalarValue>,
        executor: &'a juniper::Executor<Self::Context, juniper::DefaultScalarValue>,
    ) -> juniper::BoxFuture<'a, juniper::ExecutionResult<juniper::DefaultScalarValue>> {
        match info.fields.get(field_name) {
            Some(field) => field.resolve(executor),
            None => panic!("resolve_field not implemented for field {}", field_name),
        }
    }
}

impl juniper::GraphQLType<juniper::DefaultScalarValue> for Query {
    type Context = crate::context::Context;
    type TypeInfo = TypeInfo;

    fn name(info: &Self::TypeInfo) -> Option<&str> {
        Some(&info.name)
    }

    fn meta<'r>(
        info: &Self::TypeInfo,
        registry: &mut juniper::Registry<'r>,
    ) -> juniper::meta::MetaType<'r>
    where
        juniper::DefaultScalarValue: 'r,
    {
        let fields = &info
            .fields
            .values()
            .map(|field| field.register(registry))
            .sorted_by_key(|field| field.name.to_owned())
            .collect::<Vec<_>>();

        registry.build_object_type::<Self>(info, fields).into_meta()
    }
}

pub type Schema = juniper::RootNode<
    'static,
    Query,
    juniper::EmptyMutation<crate::context::Context>,
    juniper::EmptySubscription<crate::context::Context>,
>;

pub async fn build(config: &crate::Config) -> Schema {
    let pool = crate::connection::build(config).await;
    let introspection = introspection::Introspection::from(&pool).await;

    let fields = {
        let mut fields_builder: Vec<Box<dyn Registrable>> = vec![];

        for relation in introspection.relations() {
            fields_builder.push(Box::new(FieldInfo::<i32> {
                resolve_type: std::marker::PhantomData {},
                name: relation.name.clone(),
                resolver: |executor| {
                    let f = async move {
                        executor
                            .context()
                            .pool
                            .get()
                            .await
                            .unwrap()
                            .query_one("select 2", &[])
                            .await
                            .map(move |row| row.get::<_, i32>(0))
                            .map(juniper::Value::scalar)
                            .map_err(juniper::FieldError::from)
                    };
                    Box::pin(f)
                },
            }));
        }

        fields_builder
    };

    juniper::RootNode::new_with_info(
        Query {},
        juniper::EmptyMutation::<crate::context::Context>::new(),
        juniper::EmptySubscription::<crate::context::Context>::new(),
        TypeInfo::new("Query".into(), fields),
        (),
        (),
    )
}
