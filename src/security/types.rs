use std::fmt;

#[derive(Clone)]
pub struct RedisCommand {
    pub name: String,
    pub args: Vec<Vec<u8>>,
}

impl fmt::Debug for RedisCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisCommand")
            .field("name", &self.name)
            .field("arg_count", &self.args.len())
            .finish()
    }
}
