use crate::{
    ClientError, Config, EventContext, EventUnsubscriber, Program, ProgramAccountsIterator,
    RequestBuilder,
};
use anchor_lang::{prelude::Pubkey, AccountDeserialize, Discriminator};
#[cfg(feature = "rpc-client")]
use solana_client::{nonblocking::rpc_client::RpcClient as AsyncRpcClient, rpc_client::RpcClient};
use solana_client::{rpc_config::RpcSendTransactionConfig, rpc_filter::RpcFilterType};
use solana_sdk::{
    commitment_config::CommitmentConfig, signature::Signature, signer::Signer,
    transaction::Transaction,
};

use std::{marker::PhantomData, ops::Deref, sync::Arc};
use tokio::{
    runtime::{Builder, Handle},
    sync::RwLock,
};

impl<'a> EventUnsubscriber<'a> {
    /// Unsubscribe gracefully.
    pub fn unsubscribe(self) {
        self.runtime_handle.block_on(self.unsubscribe_internal())
    }
}

impl<C: Deref<Target = impl Signer> + Clone> Program<C> {
    pub fn new(program_id: Pubkey, cfg: Config<C>) -> Result<Self, ClientError> {
        let rt: tokio::runtime::Runtime = Builder::new_multi_thread().enable_all().build()?;

        #[cfg(not(feature = "rpc-client"))]
        return Ok(Self {
            program_id,
            cfg,
            sub_client: Arc::new(RwLock::new(None)),
            rt,
        });

        #[cfg(feature = "rpc-client")]
        {
            let comm_config = cfg.options.unwrap_or_default();
            let cluster_url = cfg.cluster.url().to_string();
            Ok(Self {
                program_id,
                cfg,
                sub_client: Arc::new(RwLock::new(None)),
                rt,
                rpc_client: RpcClient::new_with_commitment(cluster_url.clone(), comm_config),
                async_rpc_client: AsyncRpcClient::new_with_commitment(cluster_url, comm_config),
            })
        }
    }

    #[cfg(feature = "rpc-client")]
    pub fn new_with_rpc(
        program_id: Pubkey,
        cfg: Config<C>,
        rpc_client: RpcClient,
        async_rpc_client: AsyncRpcClient,
    ) -> Result<Self, ClientError> {
        let rt: tokio::runtime::Runtime = Builder::new_multi_thread().enable_all().build()?;

        Ok(Self {
            program_id,
            cfg,
            sub_client: Arc::new(RwLock::new(None)),
            rt,
            rpc_client,
            async_rpc_client,
        })
    }

    /// Returns the account at the given address.
    pub fn account<T: AccountDeserialize>(&self, address: Pubkey) -> Result<T, ClientError> {
        self.rt.block_on(self.account_internal(address))
    }

    /// Returns all program accounts of the given type matching the given filters
    pub fn accounts<T: AccountDeserialize + Discriminator>(
        &self,
        filters: Vec<RpcFilterType>,
    ) -> Result<Vec<(Pubkey, T)>, ClientError> {
        self.accounts_lazy(filters)?.collect()
    }

    /// Returns all program accounts of the given type matching the given filters as an iterator
    /// Deserialization is executed lazily
    pub fn accounts_lazy<T: AccountDeserialize + Discriminator>(
        &self,
        filters: Vec<RpcFilterType>,
    ) -> Result<ProgramAccountsIterator<T>, ClientError> {
        self.rt.block_on(self.accounts_lazy_internal(filters))
    }

    pub fn on<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
        &self,
        f: impl Fn(&EventContext, T) + Send + 'static,
    ) -> Result<EventUnsubscriber, ClientError> {
        let (handle, rx) = self.rt.block_on(self.on_internal(f))?;

        Ok(EventUnsubscriber {
            handle,
            rx,
            runtime_handle: self.rt.handle(),
            _lifetime_marker: PhantomData,
        })
    }
}

impl<'a, C: Deref<Target = impl Signer> + Clone> RequestBuilder<'a, C, Box<dyn Signer + 'a>> {
    #[cfg(not(feature = "rpc-client"))]
    pub fn from(
        program_id: Pubkey,
        cluster: &str,
        payer: C,
        options: Option<CommitmentConfig>,
        handle: &'a Handle,
    ) -> Self {
        Self {
            program_id,
            payer,
            cluster: cluster.to_string(),
            accounts: Vec::new(),
            options: options.unwrap_or_default(),
            instructions: Vec::new(),
            instruction_data: None,
            signers: Vec::new(),
            handle,
            _phantom: PhantomData,
        }
    }

    #[cfg(feature = "rpc-client")]
    pub fn from(
        program_id: Pubkey,
        cluster: &str,
        payer: C,
        options: Option<CommitmentConfig>,
        handle: &'a Handle,
        async_rpc_client: &'a AsyncRpcClient,
    ) -> Self {
        Self {
            program_id,
            payer,
            cluster: cluster.to_string(),
            accounts: Vec::new(),
            options: options.unwrap_or_default(),
            instructions: Vec::new(),
            instruction_data: None,
            signers: Vec::new(),
            handle,
            _phantom: PhantomData,
            async_rpc_client,
        }
    }

    pub fn signed_transaction(&self) -> Result<Transaction, ClientError> {
        self.handle.block_on(self.signed_transaction_internal())
    }

    pub fn send(&self) -> Result<Signature, ClientError> {
        self.handle.block_on(self.send_internal())
    }

    pub fn send_with_spinner_and_config(
        &self,
        config: RpcSendTransactionConfig,
    ) -> Result<Signature, ClientError> {
        self.handle
            .block_on(self.send_with_spinner_and_config_internal(config))
    }
}
