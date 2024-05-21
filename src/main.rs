use tokio::time::Duration;
use teloxide::{prelude::*, utils::command::BotCommands};
use tokio::process::Command as OtherCommand;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use crate::ThreadState::Running;

static STARTED: AtomicBool = AtomicBool::new(false);

enum ThreadState {
    Running = 1,
    Stopping = 2,
    Stopped = 3,
}

static STATE: AtomicI32 = AtomicI32::new(ThreadState::Stopped as i32);

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();
    let notify = Arc::new(Notify::new());
    Command::repl_with_listener(bot, |bot, msg, cmd| answer(notify.clone(), bot, msg, cmd), /* listener */).await;
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "Start service.")]
    Start,
    #[command(description = "stop service.")]
    Stop,
}

async fn worker_thread(bot: Bot, msg: Message, notify: Arc<Notify>) {
    STATE.store(Running as i32, Ordering::Relaxed);
    loop {
        if (STATE.load(Ordering::Relaxed) == ThreadState::Stopping as i32) {
            break;
        }
        let output = OtherCommand::new("dmesg")
            .arg("|")
            .arg("grep")
            .arg("WLAN_DEBUG_DFS_ALWAYS")
            .output()
            .await.expect("failed to execute process");

        let output = String::from_utf8_lossy(&output.stdout);

        if output.len() > 0 {
            bot.send_message(msg.chat.id, output).await.unwrap();
        }
        let sleep = tokio::time::sleep(Duration::from_secs(360));
        tokio::select! {
            _ = sleep => {},
            _ = notify.notified() => {},
        }
    }
    STATE.store(ThreadState::Stopped as i32, Ordering::Relaxed);
    return;
}

async fn answer(notify: Arc<Notify>, bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?,
        Command::Start => {
            let notify_clone = notify.clone();
            let botcopy = bot.clone();
            let msgcopy = msg.clone();
            tokio::spawn(async move {
                worker_thread(botcopy, msgcopy, notify_clone).await;
            });
            bot.send_message(msg.chat.id, "DFS Monitor Started.".to_string()).await?
        }
        Command::Stop => {
            STATE.store(ThreadState::Stopping as i32, Ordering::Relaxed);
            notify.notify_one();
            bot.send_message(msg.chat.id, "DFS Monitor Stopping.".to_string()).await?
        }
    };

    Ok(())
}