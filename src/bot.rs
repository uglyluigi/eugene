use serenity::{
    client::Client,
    framework::standard::{Args, StandardFramework, CommandOptions},
    model::{channel::Message, gateway::{Ready, Game}, id::UserId},
    prelude::{Context, EventHandler},
    utils::parse_mention,
    gateway::Shard,
    http,
};

use std::{
    env, 
    sync::{Arc, Mutex},
};

#[path = "facts.rs"]
mod facts;
#[path = "ttt.rs"]
mod ttt;

use ttt::{
    GameState::*,
    TicTTGame,
    Player,
};

struct Handler;

impl EventHandler for Handler {
    fn message(&self, _: Context, msg: Message) {}

    fn ready(&self, context: Context, ready_event: Ready) {
        let curr_user = &ready_event.user;

        //Load game from cfg file Eugene.toml
        context.set_game(Game::playing(&botcfg::get_str("game").expect("Game not present in Eugene.toml")));

        println!("Session ID: {}\nLogged in as: {} (descriminator = {})\n\u{1F916}? {:?}", ready_event.session_id, curr_user.name, curr_user.discriminator, curr_user.bot);
        println!(" - Ready - ");
    }
}

//The static vector that stores each in-progress game.
lazy_static! {
    static ref TTT_GAMES_MX: Mutex<Vec<TicTTGame>> = Mutex::new(vec![]);
}

pub mod botcfg {
	use config::{Config, File};
	use std::{collections::HashMap, fs::OpenOptions, io::Write, path::Path, sync::{Mutex, MutexGuard}};
	
	lazy_static! {
		static ref CONFIG: Mutex<Config> = Mutex::new(Config::default());
	}
	
	macro_rules! glock {
		( ) => {
			CONFIG.lock().expect("Failed to get config lock")
		}
	}
	
	pub fn init() {
		let	DEFAULT_VALUES = {
			let mut map = HashMap::new();
			map.insert("game", "I like money!");
			map
		};
		
		let cfg_file = Path::new("./Eugene.toml");

		if cfg_file.exists() {
			println!("Found Eugene.toml");
		} else {
			println!("Creating Eugene.toml");
			let mut file = OpenOptions::new().append(true).create(true).open("./Eugene.toml").expect("Couldn\'t create Eugene.toml");

			for (def_key, def_val) in DEFAULT_VALUES {
				file.write_all(format!("{} = \"{}\"\n", def_key, def_val).as_bytes());
			}
		}

		update();
	}
	
	fn update() {
		glock!().merge(File::with_name("Eugene.toml"));
	}
	
	pub fn get_str(key: &str) -> Option<String> {
		update();

		match glock!().get::<String>(key) {
			Ok(val) => Some(val),
			Err(why) => {
				println!("Error occurred reading property {}: {:?}", key, why);
				None
			}
		}
	}
	
	pub fn put_str(key: &str, value: String) {		
		match glock!().set(&key, value) {
			Err(why) => println!("Error assigning value {}: {:?}", key, why),
			_ => ()
		}
		
		update();
	}
}

pub fn init() {
    //Initialize the client wrapper
    let mut client = Client::new(&env::var("DISCORD_TOKEN").expect("Expected token"), Handler)
        .expect("Client creation error");

    client.with_framework(
        StandardFramework::new()
            .configure(|c| c.prefix("~"))
			.command("fact", |c| c
				.check(user_is_owner)
				.desc("Gives a random spongebob fact.")
				.cmd(fact)
			)
			.cmd("t3", t3)
    );

    if let Err(why) = client.start() {
        println!("An error occurred while running the client: {:?}", why);
    }
}

fn handleoutmsg(msg: &Message, string: String) {
    match msg.channel_id.say(&string) {
        Ok(_) => println!("Sent message: {}\nIn reply to: [{}] {:?}", string, msg.author.name, msg.content),
        Err(why) => println!("Failed to send message: {}\nError: {}", string, why),
    };
}

//Command for tic-tac-toe
command!(fact(_context, msg, args) {
	let character: String = args.single::<String>().unwrap();
	
	match facts::get_fact(character) {
		Err(why) => println!("Command error: {:?}", why),
		Ok(fact_) => {
			match msg.channel_id.say(fact_) {
				Ok(msg) => println!("[Facts] Sent message: {:?}", msg),
				Err(pourquoi) => {
					println!("[Facts] Error sending message: {:?}", pourquoi)
				}
			};
		}
	};
});

command!(t3(_context, msg, args) {
	//~tictactoe start <player1 piece> <player2-name> <player2-piece>
	println!("{}", msg.content);
	let command = args.single::<String>()?;
	let command: &str = command.as_ref();
	
	match command {
		"start" => {
			if args.len() != 4 {
				handleoutmsg(&msg, String::from("Usage: ~t3 start <player-1-emote> <player-2-name/@> <player-2-emote>"));
				return Ok(());
			}
			
			let sanitize_at = |s: String| -> String {
			match s.find('@') {
				//If the player @'s someone else to play, it must be wrapped into a UserId and resolved.
				Some(usize) => UserId(parse_mention(&s).unwrap()).get().unwrap().name,
				None => s
				}
			};

			let piece_p1 = args.single::<String>()?;
			let name_p1 = sanitize_at((&msg.author.name).to_string());
			let name_p2 = sanitize_at(args.single::<String>()?);
			let piece_p2 = args.single::<String>()?;
			let player1: Player = Player::new(name_p1, piece_p1);
			let player2: Player = Player::new(name_p2, piece_p2);
			let mut vec = &mut TTT_GAMES_MX.lock().unwrap();
			
			if let Some(_) = player_in_vec(&player1.name, vec) {
				handleoutmsg(&msg, format!("{} is already in a game!", player1.name));
				return Ok(());
			} else if let Some(_) = player_in_vec(&player2.name, vec) {
				handleoutmsg(&msg, format!("{} is already in a game!", player1.name));
				return Ok(());
			}

			handleoutmsg(&msg, format!(
				"A new game of tic tac toe has been started between {} and {}!\nUse ~t3 put <position> to choose a space on your turn. These are the positions:",
				player1.name, player2.name
			));

			handleoutmsg(&msg, format!("{}", TicTTGame::help_grid()));
			let g = TicTTGame::new(player1, player2);
			println!("{}", g);
			vec.push(g);
		},
		
		"put" => {
			if args.len() != 2 {
				handleoutmsg(&msg, String::from("Usage: ~t3 put <position>"));
				return Ok(());
			}
			
			let mut vec_mutex = TTT_GAMES_MX.lock().unwrap();

			if let Some(game) = player_in_vec(&msg.author.name, &mut vec_mutex) {
				let position = args.single::<String>()?;
				let mut target_game = game;

				match target_game.update_board(position) {
					Err(why) => {
						handleoutmsg(&msg, why.to_string());
						vec_mutex.push(target_game);
						return Ok(());
					},
					_ => ()
				};

				match target_game.state {
					Win_Player1 | Win_Player2 => handleoutmsg(&msg, format!("{} has won!", target_game.get_curr_player_mut().name)),
					Cat => handleoutmsg(&msg, String::from("\nYou both lose! Congratulations!")),
					_ => {
						let player = target_game.get_curr_player();
						handleoutmsg(&msg, format!("```\n{}```\n{} {}, it\'s your turn!", target_game.as_table(), player.name, player.piece));
						vec_mutex.push(target_game);
					}
				};
			} else {
				handleoutmsg(&msg, String::from("You are not in a game!"));
			}
		},
		
		"quit" => {
			if args.len() != 1 {
				handleoutmsg(&msg, String::from("Usage: ~t3 quit"));
				return Ok(());
			}
			
			let mut vec_mutex = TTT_GAMES_MX.lock().unwrap();
			
			//A byproduct of calling player_in_vec is removing the game from the vector so it can be moved out of the function.
			//The "put" command pushes the game back into the vector after a player makes a move. This doesn't need to happen here
			if let None = player_in_vec(&msg.author.name, &mut vec_mutex) {
				handleoutmsg(&msg, String::from("You are not in a game!"));
			} else {
				handleoutmsg(&msg, String::from("Your game has been ended early."));
			}
		},
		
		&_ => handleoutmsg(&msg, String::from("Unknown command!")),
	}
});

fn user_is_owner(_context: &mut Context, message: &Message, _: &mut Args, _: &CommandOptions) -> bool {
	message.author.id == 130013619734708224
}

fn player_in_vec(player: &String, vec: &mut Vec<TicTTGame>) -> Option<TicTTGame> {
	for index in 0..vec.len() {
		if ttt::TicTTGame::player_is_in_game(player, &vec[index]) {
			return Some(vec.remove(index));
		}
	}
	
	None
}
