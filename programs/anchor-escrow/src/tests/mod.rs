#[cfg(test)]
mod tests {

    use {
        anchor_lang::{
            AccountDeserialize, InstructionData, Key, ToAccountMetas, prelude::{msg, Clock}, solana_program::{ program_pack::Pack}
        }, anchor_spl::{
            associated_token::{
                self, 
                spl_associated_token_account
            }, 
            token::spl_token
        }, litesvm::{LiteSVM, types::TransactionMetadata}, litesvm_token::{
            CreateAssociatedTokenAccount, CreateMint, MintTo, spl_token::ID as TOKEN_PROGRAM_ID
        }, solana_account::{Account, ReadableAccount}, solana_address::Address, solana_instruction::Instruction, solana_keypair::Keypair, solana_message::Message, solana_native_token::LAMPORTS_PER_SOL, solana_pubkey::Pubkey, solana_rpc_client::rpc_client::RpcClient, solana_sdk_ids::system_program::ID as SYSTEM_PROGRAM_ID, solana_signer::Signer, solana_transaction::Transaction, std::{
            path::PathBuf, 
            str::FromStr
        }
    };

    static PROGRAM_ID: Pubkey = crate::ID;

    // Setup function to initialize LiteSVM and create a payer keypair
    // Also loads an account from devnet into the LiteSVM environment (for testing purposes)
    fn setup() -> (LiteSVM, Keypair) {
        // Initialize LiteSVM and payer
        let mut program = LiteSVM::new();
        let payer = Keypair::new();
    
        // Airdrop some SOL to the payer keypair
        program
            .airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL)
            .expect("Failed to airdrop SOL to payer");
    
        // Load program SO file
        let so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/deploy/anchor_escrow.so");
    
        let program_data = std::fs::read(so_path).expect("Failed to read program SO file");
    
        program.add_program(PROGRAM_ID, &program_data);

        // Example on how to Load an account from devnet
        // LiteSVM does not have access to real Solana network data since it does not have network access,
        // so we use an RPC client to fetch account data from devnet
        let rpc_client = RpcClient::new("https://api.devnet.solana.com");
        let account_address = Address::from_str("4YH9pgaC1a9Ff4m7Z1nCViExVLn9goV4ybqWEPaQxY27").unwrap();
        let fetched_account = rpc_client
            .get_account(&account_address)
            .expect("Failed to fetch account from devnet");

        // Set the fetched account in the LiteSVM environment
        // This allows us to simulate interactions with this account during testing
        program.set_account(payer.pubkey(), Account { 
            lamports: fetched_account.lamports, 
            data: fetched_account.data, 
            owner: Pubkey::from(fetched_account.owner.to_bytes()), 
            executable: fetched_account.executable, 
            rent_epoch: fetched_account.rent_epoch 
        }).unwrap();

        msg!("Lamports of fetched account: {}", fetched_account.lamports);
    
        // Return the LiteSVM instance and payer keypair
        (program, payer)
    }
    fn make_init(mut program: &mut LiteSVM, payer: &Keypair) -> (Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, TransactionMetadata) {

        // Get the maker's public key from the payer keypairß
        let maker = payer.pubkey();
        
        // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
        // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
        let mint_a = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint A: {}\n", mint_a);

        let mint_b = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint B: {}\n", mint_b);

        // Create the maker's associated token account for Mint A
        // This is done using litesvm-token's CreateAssociatedTokenAccount utility
        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
            .owner(&maker).send().unwrap();
        msg!("Maker ATA A: {}\n", maker_ata_a);

        // Derive the PDA for the escrow account using the maker's public key and a seed value
        let escrow = Pubkey::find_program_address(
            &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
            &PROGRAM_ID
        ).0;
        msg!("Escrow PDA: {}\n", escrow);

        // Derive the PDA for the vault associated token account using the escrow PDA and Mint A
        let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
        msg!("Vault PDA: {}\n", vault);

        // Define program IDs for associated token program, token program, and system program
        let associated_token_program = spl_associated_token_account::ID;
        let token_program = TOKEN_PROGRAM_ID;
        let system_program = SYSTEM_PROGRAM_ID;

        // Mint 1,000 tokens (with 6 decimal places) of Mint A to the maker's associated token account
        MintTo::new(&mut program, &payer, &mint_a, &maker_ata_a, 1000000000)
            .send()
            .unwrap();

        // Create the "Make" instruction to deposit tokens into the escrow
        let make_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: crate::accounts::Make {
                maker: maker,
                mint_a: mint_a,
                mint_b: mint_b,
                maker_ata_a: maker_ata_a,
                escrow: escrow,
                vault: vault,
                associated_token_program: associated_token_program,
                token_program: token_program,
                system_program: system_program,
            }.to_account_metas(None),
            data: crate::instruction::Make {deposit: 10, seed: 123u64, receive: 10 }.data(),
        };

        // Create and send the transaction containing the "Make" instruction
        let message = Message::new(&[make_ix], Some(&payer.pubkey()));
        let recent_blockhash = program.latest_blockhash();

        let transaction = Transaction::new(&[&payer], message, recent_blockhash);

        // Send the transaction and capture the result
        let tx = program.send_transaction(transaction).unwrap();

        (maker,mint_a,mint_b,maker_ata_a,escrow,vault,tx)
        
    }

    #[test]
    fn test_make() {

        // Setup the test environment by initializing LiteSVM and creating a payer keypair
        let(mut program,payer)=setup();
        let(maker,mint_a,mint_b,maker_ata_a,escrow,vault,tx)=make_init(&mut program,&payer);
        // Log transaction details
        msg!("\n\nMake transaction sucessfull");
        msg!("CUs Consumed: {}", tx.compute_units_consumed);
        msg!("Tx Signature: {}", tx.signature);

        // Verify the vault account and escrow account data after the "Make" instruction
        let vault_account = program.get_account(&vault).unwrap();
        let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
        assert_eq!(vault_data.amount, 10);
        assert_eq!(vault_data.owner, escrow);
        assert_eq!(vault_data.mint, mint_a);

        let escrow_account = program.get_account(&escrow).unwrap();
        let escrow_data = crate::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
        assert_eq!(escrow_data.seed, 123u64);
        assert_eq!(escrow_data.maker, maker);
        assert_eq!(escrow_data.mint_a, mint_a);
        assert_eq!(escrow_data.mint_b, mint_b);
        assert_eq!(escrow_data.receive, 10);
        
    }
    #[test]
    fn refund(){
        let (mut program, payer) = setup();
        let(maker,mint_a,mint_b,maker_ata_a,escrow,vault,_)=make_init(&mut program,&payer);
        let associated_token_program = spl_associated_token_account::ID;
        let token_program = TOKEN_PROGRAM_ID;
        let system_program = SYSTEM_PROGRAM_ID;

    let refund_ix=Instruction{
        program_id:PROGRAM_ID,
        accounts:crate::accounts::Refund{
            maker:maker,
            mint_a:mint_a,
            maker_ata_a:maker_ata_a,
            escrow:escrow,
            vault:vault,
            token_program:token_program,
            system_program:system_program,
        }.to_account_metas(None),
        data:crate::instruction::Refund{}.data(),   
    };
    let message = Message::new(&[refund_ix], Some(&maker));
        let recent_blockhash = program.latest_blockhash();

        let transaction = Transaction::new(&[&payer], message, recent_blockhash);

        let tx = program.send_transaction(transaction).unwrap();
        
        msg!("\n\nTake transaction sucessfull");
        msg!("CUs Consumed: {}", tx.compute_units_consumed);
        msg!("Tx Signature: {}", tx.signature);

        // After refund, vault and escrow should be closed (no lamports), and maker regains full Mint A balance
        assert_eq!(program.get_account(&vault).unwrap().lamports, 0);
        assert_eq!(program.get_account(&escrow).unwrap().lamports(), 0);

        let maker_ata_a_account = program.get_account(&maker_ata_a).unwrap();
        let maker_ata_a_data = spl_token::state::Account::unpack(&maker_ata_a_account.data).unwrap();
        assert_eq!(maker_ata_a_data.amount, 1_000_000_000);
        assert_eq!(maker_ata_a_data.mint, mint_a);
        assert_eq!(maker_ata_a_data.owner, maker);
    }
    #[test]
    fn test_take(){
        let (mut program, payer) = setup();
        let taker = Keypair::new();
        program.airdrop(&taker.pubkey(), 10 * LAMPORTS_PER_SOL).expect("airdrop taker");
        let(maker,mint_a,mint_b,maker_ata_a,escrow,vault,_)=make_init(&mut program,&payer);

        let taker_ata_a=CreateAssociatedTokenAccount::new(&mut program, &taker, &mint_a)
            .owner(&taker.pubkey()).send().unwrap();
        msg!("Taker ATA A: {}\n", taker_ata_a);

        let taker_ata_b=CreateAssociatedTokenAccount::new(&mut program, &taker, &mint_b)
            .owner(&taker.pubkey()).send().unwrap();
        msg!("Taker ATA B: {}\n", taker_ata_b);

        let maker_ata_b=CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_b)
            .owner(&maker).send().unwrap();
        msg!("Maker ATA B: {}\n", maker_ata_b);

        let associated_token_program = spl_associated_token_account::ID;
        let token_program = TOKEN_PROGRAM_ID;
        let system_program = SYSTEM_PROGRAM_ID;


        // Mint 1,000 tokens (with 6 decimal places) of Mint A to the maker's associated token account
        MintTo::new(&mut program, &payer, &mint_b, &taker_ata_b, 1000000000)
            .send()
            .unwrap();

            let take_ix=Instruction{
                program_id:PROGRAM_ID,
                accounts:crate::accounts::Take{
                    taker:taker.pubkey(),
                    maker:maker,
                    mint_a:mint_a,
                    mint_b:mint_b,
                    taker_ata_a:taker_ata_a,
                    taker_ata_b:taker_ata_b,
                    maker_ata_b:maker_ata_b,
                    escrow:escrow,
                    vault:vault,
                    associated_token_program:associated_token_program,
                    token_program:token_program,
                    system_program:system_program,
                }.to_account_metas(None),
                data:crate::instruction::Take{}.data(),
            };

        let message = Message::new(&[take_ix], Some(&taker.pubkey()));
        let recent_blockhash = program.latest_blockhash();

        let transaction = Transaction::new(&[&taker], message, recent_blockhash);

        let tx = program.send_transaction(transaction).unwrap();
        
        msg!("\n\nTake transaction sucessfull");
        msg!("CUs Consumed: {}", tx.compute_units_consumed);
        msg!("Tx Signature: {}", tx.signature);

        let vault_account = program.get_account(&vault);
        assert!(vault_account.is_none(), "vault should be closed after take");

        let taker_ata_a_account = program.get_account(&taker_ata_a).unwrap();
        let taker_ata_a_data = spl_token::state::Account::unpack(&taker_ata_a_account.data).unwrap();
        assert_eq!(taker_ata_a_data.amount, 10, "taker should have received 10 of Mint A");
        assert_eq!(taker_ata_a_data.mint, mint_a);
        assert_eq!(taker_ata_a_data.owner, taker.pubkey());

        let taker_ata_b_account = program.get_account(&taker_ata_b).unwrap();
        let taker_ata_b_data = spl_token::state::Account::unpack(&taker_ata_b_account.data).unwrap();
        assert_eq!(taker_ata_b_data.amount, 1_000_000_000 - 10, "taker should have sent 10 of Mint B");
        assert_eq!(taker_ata_b_data.mint, mint_b);
        assert_eq!(taker_ata_b_data.owner, taker.pubkey());

        let maker_ata_b_account = program.get_account(&maker_ata_b).unwrap();
        let maker_ata_b_data = spl_token::state::Account::unpack(&maker_ata_b_account.data).unwrap();
        assert_eq!(maker_ata_b_data.amount, 10, "maker should have received 10 of Mint B");
        assert_eq!(maker_ata_b_data.mint, mint_b);
        assert_eq!(maker_ata_b_data.owner, maker);
    }
     
    
    #[test]
    fn test_take_after_5days(){
        let (mut program, payer) = setup();
        let taker = Keypair::new();
        program.airdrop(&taker.pubkey(), 10 * LAMPORTS_PER_SOL).expect("airdrop taker");
        let(maker,mint_a,mint_b,maker_ata_a,escrow,vault,_)=make_init(&mut program,&payer);

        let taker_ata_a=CreateAssociatedTokenAccount::new(&mut program, &taker, &mint_a)
            .owner(&taker.pubkey()).send().unwrap();
        msg!("Taker ATA A: {}\n", taker_ata_a);

        let taker_ata_b=CreateAssociatedTokenAccount::new(&mut program, &taker, &mint_b)
            .owner(&taker.pubkey()).send().unwrap();
        msg!("Taker ATA B: {}\n", taker_ata_b);

        let maker_ata_b=CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_b)
            .owner(&maker).send().unwrap();
        msg!("Maker ATA B: {}\n", maker_ata_b);

        let associated_token_program = spl_associated_token_account::ID;
        let token_program = TOKEN_PROGRAM_ID;
        let system_program = SYSTEM_PROGRAM_ID;

        // Move the clock forward by 5 days and assert the change
        let mut clock: Clock = program.get_sysvar();
        let original_time = clock.unix_timestamp;
        let future_time_after_5days = original_time + 5 * 24 * 60 * 60;
        clock.unix_timestamp = future_time_after_5days;
        program.set_sysvar(&clock);

        let updated_clock: Clock = program.get_sysvar();
        assert!(
            updated_clock.unix_timestamp >= future_time_after_5days,
            "clock should be at least 5 days in the future"
        );

        // Mint 1,000 tokens (with 6 decimal places) of Mint A to the maker's associated token account
        MintTo::new(&mut program, &payer, &mint_b, &taker_ata_b, 1000000000)
            .send()
            .unwrap();

            let take_ix=Instruction{
                program_id:PROGRAM_ID,
                accounts:crate::accounts::Take{
                    taker:taker.pubkey(),
                    maker:maker,
                    mint_a:mint_a,
                    mint_b:mint_b,
                    taker_ata_a:taker_ata_a,
                    taker_ata_b:taker_ata_b,
                    maker_ata_b:maker_ata_b,
                    escrow:escrow,
                    vault:vault,
                    associated_token_program:associated_token_program,
                    token_program:token_program,
                    system_program:system_program,
                }.to_account_metas(None),
                data:crate::instruction::Take{}.data(),
            };

        let message = Message::new(&[take_ix], Some(&taker.pubkey()));
        let recent_blockhash = program.latest_blockhash();

        let transaction = Transaction::new(&[&taker], message, recent_blockhash);

        let tx = program.send_transaction(transaction).unwrap();
        
        msg!("\n\nTake transaction sucessfull");
        msg!("CUs Consumed: {}", tx.compute_units_consumed);
        msg!("Tx Signature: {}", tx.signature);
        
        let vault_account = program.get_account(&vault);
        assert!(vault_account.is_none(), "vault should be closed after delayed take");

        let taker_ata_a_account = program.get_account(&taker_ata_a).unwrap();
        let taker_ata_a_data = spl_token::state::Account::unpack(&taker_ata_a_account.data).unwrap();
        assert_eq!(taker_ata_a_data.amount, 10);
        assert_eq!(taker_ata_a_data.mint, mint_a);
        assert_eq!(taker_ata_a_data.owner, taker.pubkey());

        let taker_ata_b_account = program.get_account(&taker_ata_b).unwrap();
        let taker_ata_b_data = spl_token::state::Account::unpack(&taker_ata_b_account.data).unwrap();
        assert_eq!(taker_ata_b_data.mint, mint_b);
        assert_eq!(taker_ata_b_data.owner, taker.pubkey());

        let maker_ata_b_account = program.get_account(&maker_ata_b).unwrap();
        let maker_ata_b_data = spl_token::state::Account::unpack(&maker_ata_b_account.data).unwrap();
        assert_eq!(maker_ata_b_data.amount, 10);
        assert_eq!(maker_ata_b_data.mint, mint_b);
        assert_eq!(maker_ata_b_data.owner, maker);
    }

}