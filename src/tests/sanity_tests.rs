// very very basic tests to just see that the harness is working correctly

use std::time::Duration;

// do NOT use MINECRAFT_TESTING_ENV directly!

use futures::future::join_all;

use crate::{
    minecraft::{types::*, vanilla::{block_type::MinecraftBlock, data_globals::get_mc_data}},
    tests::test_harness::{
        ComputerConfigs, ComputerKind, ComputerSetup, MinecraftTestHandle, TestArea, TestCommand,
    },
};

#[tokio::test]
/// Make a herobrine spawner
async fn basic_block_test() {
    let area = TestArea {
        size_x: 5,
        size_z: 5,
    };

    // gold base
    let base = TestCommand::Fill(
        MinecraftPosition {
            x: 1,
            y: 1,
            z: 1,
            facing: None,
        },
        MinecraftPosition {
            x: 3,
            y: 1,
            z: 3,
            facing: None,
        },
        MinecraftBlock::from_string("gold_block").unwrap(),
    );

    // netherrack
    let rack = TestCommand::SetBlock(
        MinecraftPosition {
            x: 2,
            y: 2,
            z: 2,
            facing: None,
        },
        MinecraftBlock::from_string("netherrack").unwrap(),
    );

    // fire
    let fire = TestCommand::SetBlock(
        MinecraftPosition {
            x: 2,
            y: 3,
            z: 2,
            facing: None,
        },
        MinecraftBlock::from_string("fire").unwrap(),
    );

    // torch1
    let torch1 = TestCommand::SetBlock(
        MinecraftPosition {
            x: 1,
            y: 2,
            z: 2,
            facing: None,
        },
        MinecraftBlock::from_string("redstone_torch").unwrap(),
    );
    // torch2
    let torch2 = TestCommand::SetBlock(
        MinecraftPosition {
            x: 2,
            y: 2,
            z: 1,
            facing: None,
        },
        MinecraftBlock::from_string("redstone_torch").unwrap(),
    );
    // torch3
    let torch3 = TestCommand::SetBlock(
        MinecraftPosition {
            x: 3,
            y: 2,
            z: 2,
            facing: None,
        },
        MinecraftBlock::from_string("redstone_torch").unwrap(),
    );
    // torch4
    let torch4 = TestCommand::SetBlock(
        MinecraftPosition {
            x: 2,
            y: 2,
            z: 3,
            facing: None,
        },
        MinecraftBlock::from_string("redstone_torch").unwrap(),
    );

    let build_commands: Vec<TestCommand> = vec![base, rack, fire, torch1, torch2, torch3, torch4];

    let mut test = MinecraftTestHandle::new(area).await;

    // build the spawner
    for cmd in build_commands {
        assert!(test.command(cmd).await);
    }

    // Check that the fire ended up in the correct position.
    let fire_check = TestCommand::TestForBlock(
        MinecraftPosition {
            x: 2,
            y: 3,
            z: 2,
            facing: None,
        },
        MinecraftBlock::from_string("fire").unwrap(),
    );

    assert!(test.command(fire_check).await);

    // if we made it here, the test has passed.
    test.stop(true).await;
}

#[tokio::test]
/// Place every block
async fn place_every_block() {
    let area = TestArea {
        size_x: 3,
        size_z: 3,
    };

    let mut test = MinecraftTestHandle::new(area).await;

    let block_pos = MinecraftPosition {
        x: 1,
        y: 1,
        z: 1,
        facing: None,
    };
    let data = get_mc_data();
    
    assert!(data.blocks_by_name.keys().len() != 0);
    let mut failed: bool = false;
    let mut failed_block: String = "".to_string();
    for block in data.blocks_by_name.keys() {
        if !test.command(TestCommand::SetBlock(block_pos, MinecraftBlock::from_string(block).unwrap())).await {
            failed = true;
            failed_block = block.to_string();
            break
        };
    }
    test.stop(!failed).await;
    if failed {
        panic!("Failed to place {failed_block} !")
    }
}

#[tokio::test]
/// Place a computer that does nothing.
async fn basic_computer_test() {
    let area = TestArea {
        size_x: 3,
        size_z: 3,
    };

    // The facing direction of this computer does not matter.
    let computer_position = MinecraftPosition {
        x: 1,
        y: 1,
        z: 1,
        facing: None,
    };

    let computer = ComputerSetup::new(
        ComputerKind::Basic,
        ComputerConfigs::Empty,
        computer_position,
        MinecraftFacingDirection::North,
    );

    let setup_commands: Vec<TestCommand> = vec![];

    let computers: Vec<ComputerSetup> = vec![computer];
    todo!("Bring back computers!");

    let test = MinecraftTestHandle::new(area).await;
}

#[tokio::test]
/// Place a computer that reaches out to a websocket and yells into it.
async fn basic_websocket_test() {
    todo!()
}
