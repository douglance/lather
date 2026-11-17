use crate::model::Result;
use incurs::command::{CommandDef, TypedResult};
use incurs::output::{CtaBlock, CtaEntry};
use incurs::schema::IncurSchema;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
struct Action {
    command: String,
    reason: String,
}

#[derive(Serialize, JsonSchema)]
pub struct Reply<T> {
    schema_version: u32,
    data: T,
    next_actions: Vec<Action>,
    #[serde(skip)]
    pub exit_code: Option<i32>,
}

impl<T> Reply<T> {
    pub fn new(data: T, command: String, reason: &str) -> Self {
        Self {
            schema_version: 1,
            data,
            next_actions: vec![Action {
                command,
                reason: reason.into(),
            }],
            exit_code: None,
        }
    }
}

pub fn command<A, O, T>(
    name: &str,
    alias: Option<&str>,
    description: &str,
    handler: fn(A, O) -> Result<Reply<T>>,
) -> CommandDef
where
    A: IncurSchema + Send + Sync + 'static,
    O: IncurSchema + Send + Sync + 'static,
    T: JsonSchema + Serialize + Send + Sync + 'static,
{
    CommandDef::typed::<A, O, (), Reply<T>, _, _>(name, move |ctx| async move {
        match tokio::task::spawn_blocking(move || handler(ctx.args, ctx.options)).await {
            Ok(Ok(data)) => {
                let commands = data
                    .next_actions
                    .iter()
                    .map(|action| CtaEntry::Detailed {
                        command: action.command.clone(),
                        description: Some(action.reason.clone()),
                    })
                    .collect();
                TypedResult::Ok {
                    exit_code: data.exit_code,
                    data,
                    cta: Some(CtaBlock {
                        commands,
                        description: None,
                    }),
                }
            }
            Ok(Err(error)) => TypedResult::error(error.code, error.message),
            Err(error) => TypedResult::error("WORKER_ERROR", error.to_string()),
        }
    })
    .description(description)
    .command_aliases(alias.into_iter().map(str::to_owned).collect())
    .done()
}
