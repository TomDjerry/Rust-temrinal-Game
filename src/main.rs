mod game;

fn main() -> anyhow::Result<()> {
    let config = game::GameConfig::from_args();
    game::run(config)
}
