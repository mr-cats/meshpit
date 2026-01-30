// Testing galore

use crate::minecraft::{types::*, vanilla::block_type::MinecraftBlock};
use std::{cmp::max, marker::PhantomData, sync::Arc, time::Duration};

use log::info;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use crate::tests::bridge::MinecraftEnvironment;

pub struct MinecraftTestEnvironment {
    /// The Minecraft environment we're running in.
    pub environment: MinecraftEnvironment,
    /// Where the current offset of the test setup plots are
    next_plot_corner: MinecraftPosition,
    /// The highest z value we've seen, as to not crash into old tests.
    highest_z: i64,
}

// this is the minecraft instance that all of the tests will use, so we have to make it once and then hold it here, otherwise
// we would have to re-start the server for every test, and that would be stupid as hell.
// TODO: I think i can refactor this into that ctor startup block that sets up logging for tests.
pub static MINECRAFT_TESTING_ENV: Lazy<Arc<Mutex<MinecraftTestEnvironment>>> = Lazy::new(|| {
    let env = MinecraftTestEnvironment::setup();
    Arc::new(env.into())
});

impl MinecraftTestEnvironment {
    /// Make a new test environment.
    ///
    /// This references the normal Minecraft environment, so this gets built after the first environment builds.
    fn setup() -> Self {
        let handle = std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Should be able to spawn a runtime for lazy making.");

            rt.block_on(async { MinecraftEnvironment::start().await })
        });

        let environment = handle.join().expect("well crap");

        // Load and lock that global, its ours!
        Self {
            environment,
            // This does not need a facing direction.
            next_plot_corner: MinecraftPosition {
                x: 0,
                y: -60,
                z: 0,
                facing: None,
            },
            highest_z: 0
        }
    }
}

/// The area that a test occupies. Will make a floor under the test of the size specified.
#[derive(Clone, Copy)]
pub struct TestArea {
    /// How many blocks long (east direction) this test needs
    pub size_x: u16,
    /// How many blocks wide (south direction) this test needs
    pub size_z: u16,
}

/// Minecraft commands that tests are able to run.
///
/// All positions input into this command are interpreted as offsets from 0,0,0.
pub enum TestCommand {
    /// Place a block.
    SetBlock(MinecraftPosition, MinecraftBlock),
    /// Fill some blocks.
    Fill(MinecraftPosition, MinecraftPosition, MinecraftBlock),
    /// Test for a block at some position
    ///
    TestForBlock(MinecraftPosition, MinecraftBlock),
}

impl TestCommand {
    /// Runs a supplied command. Returns the string resulting from the command.
    ///
    /// Returns true if the command succeeded.
    async fn invoke(&self, handle: &mut MinecraftTestHandle) -> bool {
        // Get the environment so we can run commands
        let mut env = MINECRAFT_TESTING_ENV.lock().await;
        let corner = handle.corner;
        match self {
            TestCommand::SetBlock(minecraft_position, minecraft_block) => {
                // TODO: support facing directions. maybe facing should go into the MinecraftBlock?
                let position = corner.with_offset(*minecraft_position).as_command_string();
                let block_string = minecraft_block.get_full_name();
                let result = env
                    .run_command(format!("setblock {position} {block_string}"))
                    .await;
                // If we placed the block, we will get this text.
                result.contains("Changed the block")
            }
            TestCommand::Fill(pos1, pos2, minecraft_block) => {
                let position1 = corner.with_offset(*pos1).as_command_string();
                let position2 = corner.with_offset(*pos2).as_command_string();
                let block_string = minecraft_block.get_full_name();
                let result = env
                    .run_command(format!("fill {position1} {position2} {block_string}"))
                    .await;
                // Should fill
                result.contains("Successfully filled")
            }
            TestCommand::TestForBlock(minecraft_position, minecraft_block) => {
                // This command is less trivial.
                // To check if a block exists without replacing it, we need to use a execute command to test the block.
                // However, execute will not output anything, so we need to mark that the test passed or failed by placing
                // another block to test against. This time outside of the test.
                // Thus we use the block at the corner, offset -1 to move out of the test bounds, then fill that.
                let position_to_check = corner.with_offset(*minecraft_position).as_command_string();
                let marking_position = corner
                    .with_offset(MinecraftPosition {
                        x: -1,
                        y: 0,
                        z: -1,
                        facing: None,
                    })
                    .as_command_string();
                let check_block = minecraft_block.get_full_name();
                let marking_block = MinecraftBlock::from_string("bedrock")
                    .unwrap()
                    .get_full_name();

                // We will use the execute command to check the position for the block we want, and if it is there, we will place
                // the marking block.
                // execute if block ? ? ? (the block we're checking for) run setblock ? ? ? minecraft:bedrock
                let check_command = format!(
                    "execute if block {position_to_check} {check_block} run setblock {marking_position} {marking_block}"
                );

                // This will have no result.
                let _ = env.run_command(check_command).await;

                // Now we check for the marking block by trying to place a block there with `keep`, since its only allowed to replace air.
                // Thus if we are able to place the marking block there, it must have been air, and thus the marker block is missing.
                let mark_check_command: String =
                    format!("/setblock {marking_position} {marking_block} keep");

                let check_passed = env
                    .run_command(mark_check_command)
                    .await
                    .contains("Could not set the block");

                // now regardless if that block is there or not, clean up afterwards
                let _ = env
                    .run_command(format!("/setblock {marking_position} air"))
                    .await;

                check_passed
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct ComputerSetup {
    /// What's the ID of this computer?
    ///
    /// This is set automatically when they are placed in the world.
    id: Option<u16>,
    /// What kind of computer is this?
    kind: ComputerKind,
    /// Which config to use when creating this computer
    config: ComputerConfigs,
    /// Relative position within the testcase
    offset: MinecraftPosition,
    /// Facing direction
    facing: MinecraftFacingDirection,
}

impl ComputerSetup {
    /// Make a new computer setup for a test
    pub fn new(
        kind: ComputerKind,
        config: ComputerConfigs,
        offset: MinecraftPosition,
        facing: MinecraftFacingDirection,
    ) -> Self {
        Self {
            id: None, // This will be set later.
            kind,
            config,
            offset,
            facing,
        }
    }
}

// TODO: get this out of here into the computercraft folder
#[derive(Clone, Copy)]
pub enum ComputerKind {
    /// Normal computer.
    Basic,
    /// A turtle. Also configures the fuel it starts with
    Turtle(TurtleFuelSetup),
}

impl From<ComputerKind> for MinecraftBlock {
    fn from(value: ComputerKind) -> Self {
        match value {
            ComputerKind::Basic => MinecraftBlock::from_string("computer_normal").unwrap(),
            ComputerKind::Turtle(_) => MinecraftBlock::from_string("turtle_normal").unwrap(),
        }
    }
}

#[derive(Clone, Copy)]
pub enum TurtleFuelSetup {
    /// No fuel (you wont be able to move!)
    Empty,
    /// Gives the turtle one block of coal (800 fuel).
    ///
    /// Does not call turtle.refuel()
    Fueled,
}

#[derive(Clone, Copy)]
pub enum ComputerConfigs {
    /// Do not pre-setup this computer at all.
    Empty,
    /// Open a websocket and execute any lua code given
    Websocket(u16),
}

impl ComputerConfigs {
    fn to_file(self) -> Vec<u8> {
        match self {
            ComputerConfigs::Empty => include_bytes!("../tests/startup_scripts/empty.lua").to_vec(),
            ComputerConfigs::Websocket(socket) => {
                let file = include_str!("../tests/startup_scripts/eval_socket.lua");
                let mut fixed = file.replace("###URL###", "localhost"); //TODO: make the address configurable
                fixed = fixed.replace("###SOCKET###", &socket.to_string());
                fixed.as_bytes().into()
            }
        }
    }
}

/// A handle of a running test.
pub struct MinecraftTestHandle {
    /// The area that this test occupies, and the corner that the test is based on.
    ///
    /// Test areas start at the southeast corner position and grow from there. (towards positive x/z)
    ///
    /// This cannot be modified once the test has started, since we cannot re-allocate the plot we are running on.
    area: TestArea,

    /// The south-east corner of the test area. All coordinates used in tests are relative to this position.
    corner: MinecraftPosition,
}

impl MinecraftTestHandle {
    /// Create the testing plot for this test and return a test handle.
    ///
    /// Requires a test area, since we need to know how much space this test will use.
    pub async fn new(area: TestArea) -> Self {
        // Account for the area starting at "0" thus being 1 block bigger than expected.
        let area = TestArea {
            size_x: area.size_x - 1,
            size_z: area.size_x - 1,
        };

        // Get a copy of the environment
        let mut env = MINECRAFT_TESTING_ENV.lock().await;

        // Find our plot position
        let corner = env.get_test_position(area);

        // Force load the plot
        let c1 = corner;
        let offset = MinecraftPosition {
            x: area.size_x.into(),
            y: 0,
            z: area.size_z.into(),
            facing: None,
        };
        let c2 = corner.with_offset(offset);

        let command: String = format!("forceload add {} {} {} {}", c1.x, c1.z, c2.x, c2.z,);
        let _ = env.run_command(command).await; // we assume this works, also i dont wanna parse the output rn

        // Update the floor
        env.update_floor(
            corner,
            area,
            MinecraftBlock::from_string("yellow_concrete").unwrap(),
        )
        .await;

        Self { area, corner }
    }

    /// Run a test command.
    pub async fn command(&mut self, command: TestCommand) -> bool {
        command.invoke(self).await
    }

    /// Finish the test and clean up. Requires a pass or fail status to update the plot floor.
    pub async fn stop(self, passed: bool) {
        let mut env = MINECRAFT_TESTING_ENV.lock().await;
        // Update floor
        let block = if passed {
            MinecraftBlock::from_string("lime_concrete").unwrap()
        } else {
            MinecraftBlock::from_string("red_concrete").unwrap()
        };

        env.update_floor(self.corner, self.area, block).await;
        // We do not stop force-loading the chunks, since another test could be contained within it.
        // If we had something else to clean here, we would. but we dont.
        // TODO: Turn off all the computers within this chunk if possible? Maybe replace their blocks if needed.
    }
}

// we can turn on turtles with nbt set commands
// /data modify block { coords } On set value 1b
// or computercraft commands
// /computercraft turn-on [computers...]

// get the ID of a turtle
// /data get block 1 -60 2 ComputerId

// make sure to always forceload the chunks that the test is in
// forceload add <from> [<to>]

// you can put items into turtles with hoppers

//TODO: test if the test escapes the boundaries of the test.

// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT
// TODO: REMOVE ALL THIS RANDOM JUNK FROM MINECRAFT TEST ENVIRONMENT

impl MinecraftTestEnvironment {
    /// Runs a minecraft command. Does not need a preceding slash.
    async fn run_command(&mut self, command: String) -> String {
        self.environment
            .send_rcon(&command)
            .await
            .expect("Rcon should be open.")
    }

    // /// Build a computer within a test
    // async fn setup_computers(&mut self, test: &mut MinecraftTest) {
    //     let computer_directory = self
    //         .environment
    //         .get_server_folder()
    //         .join("world/computercraft/computer");
    //     for c in &mut test.computers {
    //         // TODO: refactor things into ComputerSetup
// 
    //         // place the computer
    //         // /setblock { coords } computercraft:turtle_normal[facing=east]
    //         let computer_position = test
    //             .corner
    //             .expect("Should be set by now")
    //             .with_offset(c.offset);
    //         let computer_position_string = computer_position.as_command_string();
    //         let as_block: MinecraftBlock = c.kind.into();
    //         let computer_block = as_block.get_full_name();
    //         let facing = c.facing.to_string();
    //         let command =
    //             format!("setblock {computer_position_string} {computer_block}[facing={facing}]");
    //         let result = self.run_command(command).await;
    //         info!("{result}");
// 
    //         // turn on the computer so it grabs an ID
    //         // /data modify block { coords } On set value 1b
    //         let command_2 = format!("data modify block {computer_position_string} On set value 1b");
    //         let result = self.run_command(command_2).await;
    //         info!("{result}");
// 
    //         // Get the ID of that new computer
    //         // /data get block { coords } ComputerId
    //         let command_3 = format!("data get block {computer_position_string} ComputerId");
    //         let result = self.run_command(command_3).await;
    //         info!("{result}");
    //         let id: u16 = result
    //             .split(':')
    //             .next_back()
    //             .expect("Should be one, unless placing computer failed.")
    //             .trim()
    //             .parse()
    //             .expect("Should fit.");
// 
    //         // and turn the computer off again. lol.
    //         // setting the block data here does not actually turn off the computer, so we have to use the
    //         // comptuercraft command instead.
    //         // /computercraft shutdown 0
    //         let command_4 = format!("computercraft shutdown {id}");
    //         let result = self.run_command(command_4).await;
    //         info!("{result}");
// 
    //         // update the id
    //         c.id = Some(id);
// 
    //         // if this is a turtle that wants fuel, put it in.
    //         // TODO: PLEASE PULL THIS INTO ITS OWN THING I BEG
    //         match &c.kind {
    //             ComputerKind::Turtle(fuel) => {
    //                 match fuel {
    //                     TurtleFuelSetup::Empty => { /* nothing to do */ }
    //                     TurtleFuelSetup::Fueled => {
    //                         // place a hopper above the turtle
    //                         let up_one = MinecraftPosition {
    //                             x: 0,
    //                             y: 1,
    //                             z: 0,
    //                             facing: None,
    //                         };
    //                         let offset_1 = computer_position.with_offset(up_one);
    //                         let offset_2 = offset_1.with_offset(up_one);
// 
    //                         let place_hopper = format!(
    //                             "setblock {} minecraft:hopper",
    //                             offset_1.as_command_string()
    //                         );
    //                         let result = self.environment.send_rcon(&place_hopper).await.unwrap();
    //                         info!("{result}");
// 
    //                         // now spawn the coal item lol
    //                         // /summon item { coords } {Item:{id:coal_block,count:1,components:{}}}
    //                         let spawn_coal = format!(
    //                             "summon item {} {{Item:{{id:coal_block,count:1,components:{{}}}}}}",
    //                             offset_2.as_command_string()
    //                         );
    //                         let result = self.environment.send_rcon(&spawn_coal).await.unwrap();
    //                         info!("{result}");
// 
    //                         // wait for the coal to go in
    //                         std::thread::sleep(Duration::from_secs(1));
// 
    //                         // goodbye hopper
    //                         let murder =
    //                             format!("setblock {} minecraft:air", offset_1.as_command_string());
    //                         let result = self.environment.send_rcon(&murder).await.unwrap();
    //                         info!("{result}");
    //                     }
    //                 }
    //             }
    //             _ => { /* doesn't need anything else done */ }
    //         }
// 
    //         // Now set up the lua it runs
    //         // make the folder for the computer
    //         let this_computer_dir = computer_directory.join(id.to_string());
    //         std::fs::create_dir_all(&this_computer_dir).unwrap();
    //         std::fs::write(this_computer_dir.join("startup.lua"), c.config.to_file()).unwrap();
// 
    //         // now turn on the computer again
    //         // we know the ID now so we can just use the computercraft command.
    //         // /computercraft turn-on 0
    //         let command_5 = format!("computercraft turn-on {id}");
    //         let result = self
    //             .environment
    //             .send_rcon(&command_5)
    //             .await
    //             .expect("Computer should be there");
    //         info!("{result}");
// 
    //         // all done making the computer :D
    //     }
    // }

    /// Change the floor of a test
    async fn update_floor(
        &mut self,
        corner: MinecraftPosition,
        area: TestArea,
        floor_block: MinecraftBlock,
    ) {
        let p1 = corner.as_command_string();
        let p2 = MinecraftPosition {
            x: corner.x + i64::from(area.size_x),
            y: corner.y,
            z: corner.z + i64::from(area.size_z),
            facing: None,
        }
        .as_command_string();
        let block = floor_block.get_full_name();
        // run the fill command
        let command = format!("fill {p1} {p2} {block}");
        let result = self.environment.send_rcon(&command).await;
        if let Some(feedback) = result {
            if feedback.contains("is not loaded") {
                panic!("Tried to fill outside of loaded chunks!");
            }
            if feedback.contains("Too many blocks") {
                panic!("Floor for test is WAY too big!");
            }
        }
    }

    /// Get the next open test position
    fn get_test_position(&mut self, area: TestArea) -> MinecraftPosition {
        // we just give the current position, then we update our offsets afterwards
        // to pre-prepare for the next test
        let give_me = self.next_plot_corner;

        // Update the z gap if needed
        self.highest_z = max(self.highest_z, area.size_z.into());

        let test_gap: i64 = 4;

        // Set the corner of where the next plot will be
        // If the last test was over 100 blocks away from 0,0 on the x axis, we will
        // move to a new row of tests. since we wanna look at em :D

        let mut next_x: i64 = give_me.x;
        let mut next_z: i64 = give_me.z;

        if next_x >= 100 {
            // Shift down by the highest width seen, plus the gap
            next_z += self.highest_z + test_gap;

            // Reset the x position, and shift back since we always add the offset
            // regardless if we moved to a new row.
            next_x = 0 - test_gap - area.size_x as i64;

            // Reset the largest z we've seen
            self.highest_z = 0
        }

        // Increment to the next plot
        next_x += area.size_x as i64 + test_gap;
        let next: MinecraftPosition = MinecraftPosition {
            x: next_x,
            y: give_me.y, // the y position never changes.
            z: next_z,
            facing: None,
        };
        self.next_plot_corner = next;

        give_me
    }
}
