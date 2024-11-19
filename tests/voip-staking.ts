import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { VoipStaking } from "../target/types/voip_staking";
import { BN } from "bn.js";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, Signer, SystemProgram } from "@solana/web3.js";
import { createMint, getOrCreateAssociatedTokenAccount, mintTo, transfer, TOKEN_PROGRAM_ID } from '@solana/spl-token';


const TestProgram = async() => {
  console.log("-------------------------------SET UP BEGIN-----------------------------------");
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.VoipStaking as Program<VoipStaking>;

  const mint = Keypair.generate();
  const user = Keypair.generate();
  const userSig: Signer = {
    publicKey: user.publicKey,
    secretKey: user.secretKey
  }
  const admin = Keypair.generate();
  const adminSig: Signer = {
    publicKey: admin.publicKey,
    secretKey: admin.secretKey
  }

  await program.provider.connection.confirmTransaction(
    await program.provider.connection.requestAirdrop(
      admin.publicKey,
      3 * LAMPORTS_PER_SOL
    ),
    "confirmed"
  );

  await program.provider.connection.confirmTransaction(
    await program.provider.connection.requestAirdrop(
      program.programId,
      3 * LAMPORTS_PER_SOL
    ),
    "confirmed"
  );

  await program.provider.connection.confirmTransaction(
    await program.provider.connection.requestAirdrop(
      mint.publicKey,
      3 * LAMPORTS_PER_SOL
    ),
    "confirmed"
  );

  await program.provider.connection.confirmTransaction(
    await program.provider.connection.requestAirdrop(
      user.publicKey,
      3 * LAMPORTS_PER_SOL
    ),
    "confirmed"
  );

  const STATE_SEED = "state";
  const STAKE_INFO_SEED = "stake_info";

  const [state, _a] = PublicKey.findProgramAddressSync(
    [
      Buffer.from(STATE_SEED),
    ],
    
    program.programId
  );

  const [stakeInfo, _b] = PublicKey.findProgramAddressSync(
    [
      Buffer.from(STAKE_INFO_SEED),
      user.publicKey.toBuffer()
    ],
    program.programId
  )

  console.log("-------------------------------SET UP COMPLETE-----------------------------------");
  console.log("-----------------------USER ADDRESS: ", user.publicKey.toBase58());
  console.log("-----------------------ADMIN ADDRESS: ", admin.publicKey.toBase58());
  console.log("-----------------------STATE ADDRESS: ", state.toBase58());
  console.log("-----------------------PROGRAM ID: ", program.programId.toBase58());

  console.log("-------------------------------INITIALIZATION BEGIN-----------------------------------");
  const info = await program.provider.connection.getAccountInfo(state);
  if (!info) {
    console.log("  State not found. Initializing Program...");

    const initContext = {
      state: state,
      admin: admin.publicKey,
      systemProgram: SystemProgram.programId,
    };

    const initTxHash = await program.methods.initialize().accounts(initContext).signers([adminSig]).rpc();
    await program.provider.connection.confirmTransaction(initTxHash, "finalized");
    console.log("Initialize transaction signature", initTxHash);
  } else {    
    // Do not attempt to initialize if already initialized
    console.log("  State already found.");
    console.log("  State Address: ", state.toBase58());
  }
  console.log("-------------------------------INITIALIZATION COMPLETE-----------------------------------");

  console.log("-------------------------------PAUSE BEGIN-----------------------------------");
  const pauseContext = {
    state: state,
    admin: admin.publicKey,
  };

  const pauseTxHash = await program.methods.pause().accounts(pauseContext).signers([adminSig]).rpc();
  await program.provider.connection.confirmTransaction(pauseTxHash, "finalized");
  console.log("Pause transaction signature", pauseTxHash);

  console.log("-------------------------------PAUSE COMPLETE-----------------------------------");

  console.log("-------------------------------UN-PAUSE BEGIN-----------------------------------");
  const unPauseContext = {
    state: state,
    admin: admin.publicKey,
  };

  const unPauseTxHash = await program.methods.unPause().accounts(unPauseContext).signers([adminSig]).rpc();
  await program.provider.connection.confirmTransaction(unPauseTxHash, "finalized");
  console.log("Unpause transaction signature", unPauseTxHash);

  console.log("-------------------------------UN-PAUSE COMPLETE-----------------------------------");

  console.log("-------------------------------STAKE BEGIN-----------------------------------");
  const connection = new Connection(
    'http://127.0.0.1:8899', "confirmed"
  )

  // create test token
  const token = await createMint(
    connection,
    mint,
    mint.publicKey,
    null,
    9
  );

  console.log("------------------------------TOKEN MINT ADDRESS: ", token.toBase58());

  const tokenAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    mint,
    token,
    mint.publicKey
  )

  await mintTo(
    connection,
    mint,
    token,
    tokenAccount.address,
    mint,
    1000000 * 10 ** 9 // mint 1000
  )

  const userAta = await getOrCreateAssociatedTokenAccount(connection, admin, token, user.publicKey);
  const contractAta = await getOrCreateAssociatedTokenAccount(connection, admin, token, state, true);
  
  // transfer token to user account
  try {
    const fundUserTxHash = await transfer(
      connection,
      mint,
      tokenAccount.address,
      userAta.address,
      mint.publicKey,
      10000 * 10 ** 9 
    );
    await program.provider.connection.confirmTransaction(fundUserTxHash, "finalized");
  } catch (error) {
    console.error(`Failed to fund user with ${error}`)
  }

  const stakeContext = {
    stakeInfo: stakeInfo,
    state: state,
    userAta: userAta.address,
    contractAta: contractAta.address,
    user: userSig.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
    associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID
  }; 

  const stakeAmount = 10000 * 10 ** 9;
  const stakeTime = { oneHundredDays: "OneHundredDays" } as never;
  const stakeTxHash = await program.methods.stake(new BN(stakeAmount), stakeTime ).accounts(stakeContext).signers([userSig]).rpc();
  await program.provider.connection.confirmTransaction(stakeTxHash, "finalized");
  console.log("Stake transaction signature", stakeTxHash);
  console.log("-------------------------------STAKE COMPLETE-----------------------------------");

  console.log("-------------------------------CLAIM BEGIN-----------------------------------");
  console.log("------------------------------STATE ATA: ", contractAta.address.toBase58());
  // transfer token to contract account
  try {
    const fundingTx = await transfer(
      connection,
      mint,
      tokenAccount.address,
      contractAta.address,
      mint.publicKey,
      10000 * 10 ** 9 
    );
    await program.provider.connection.confirmTransaction(fundingTx, "finalized");
  } catch (error) {
    console.error(`Failed to fund contract with ${error}`);
  }

  const claimContext = {
    stakeInfo: stakeInfo,
    state: state,
    userAta: userAta.address,
    contractAta: contractAta.address,
    user: userSig.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
    associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID
  }; 
  const claimTxHash = await program.methods.claim().accounts(claimContext).signers([userSig]).rpc();
  await program.provider.connection.confirmTransaction(claimTxHash, "finalized");
  console.log("Claim transaction signature", claimTxHash);

  console.log("-------------------------------CLAIM COMPLETE-----------------------------------");

  console.log("-------------------------------WITHDRAW BEGIN-----------------------------------");
  const withdrawContext = {
    stakeInfo: stakeInfo,
    state: state,
    userAta: userAta.address,
    contractAta: contractAta.address,
    user: userSig.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
    associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID
  }; 
  const withdrawTxHash = await program.methods.withdraw().accounts(withdrawContext).signers([userSig]).rpc();
  await program.provider.connection.confirmTransaction(withdrawTxHash, "finalized");
  console.log("Withdraw transaction signature", withdrawTxHash);

  console.log("-------------------------------WITHDRAW COMPLETE-----------------------------------");
};

const sleep = (ms = 0) =>
  new Promise(resolve => setTimeout(resolve, ms));

const runTest = async () => {
  try {
    await TestProgram();
    process.exit(0);
  } catch (error) {
    console.error(error);
    process.exit(1);
  }
}

runTest()
