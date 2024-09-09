# How to set up an InvArch or Tinkernet Collator

Make sure you are logged in as a user with root privileges.

Create a new service user to run your collator service:
```shell
sudo useradd --no-create-home --shell /usr/sbin/nologin tinkernet-collator
```

You can either install the InvArch node using a pre-built binary from GitHub, or compile it from source if you'd like to customize it.

## Standard Installation

Download the pre-built binary to `/usr/local/bin` and give it the necessary permissions & ownership:
```shell
sudo wget -O /usr/local/bin/tinkernet-collator https://github.com/Abstracted-Labs/InvArch/releases/latest/download/invarch-collator
sudo chmod +x /usr/local/bin/tinkernet-collator
sudo chown tinkernet-collator:tinkernet-collator /usr/local/bin/tinkernet-collator
```


## Install from Source

Install some required stuff:

```shell
curl https://sh.rustup.rs -sSf | sh
```
(choose option 1 - Proceed with installation (default))
You may need to restart your system before the next steps.
```
sudo apt -y install cmake git clang libclang-dev
```

Clone the repo:

 ```shell
 git clone git@github.com:Abstracted-Labs/InvArch.git
 ```

Make sure you can see the "InvArch" folder using `ls -la`. If not then you likely did something wrong.

Enter the repo and check out the most recent tagged release of code (https://github.com/Abstracted-Labs/InvArch/releases)

```shell
cd InvArch
git checkout $(git describe --tags $(git rev-list --tags --max-count=1))
```

You need to compile the code, this will take quite a while depending on your system (30+ minutes is normal):
```
cargo build --release --features tinkernet
```

Move the node executable to `usr/local/bin`, make it executable, and change ownership to our `tinkernet-collator` service user:
```shell
sudo mv ~/InvArch/target/release/invarch-collator /usr/local/bin/tinkernet-collator
sudo chmod +x /usr/local/bin/tinkernet-collator
sudo chown tinkernet-collator:tinkernet-collator /usr/local/bin/tinkernet-collator
```

## Run the node

Now that you've got a collator node executable at `/usr/local/bin/tinkernet-collator` (either pre-built or built yourself), you're ready to create its data directory and run it for the first time.

Download the chainspec (`tinker-raw.json`), set up the `tinkernet` data directory, and give it the necessary ownership:
```shell
sudo mkdir /var/lib/tinkernet
sudo wget -O /var/lib/tinkernet/tinker-raw.json https://github.com/Abstracted-Labs/InvArch/releases/latest/download/tinker-raw.json
sudo chown -R tinkernet-collator:tinkernet-collator /var/lib/tinkernet
```


Create a systemd service file to run your collator (and automatically restart it):

```shell
sudo nano /etc/systemd/system/tinkernet-collator.service
```

Within that file, paste in the following:
```
[Unit]
Description=Tinkernet (InvArch) Collator
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=tinkernet-collator
Group=tinkernet-collator
ExecStart=/usr/local/bin/tinkernet-collator \
  --base-path /var/lib/tinkernet \
  --collator \
  --force-authoring \
  --name "YOUR-COLLATOR-NAME-HERE" \
  --chain /var/lib/tinkernet/tinker-raw.json \
  --listen-addr "/ip4/0.0.0.0/tcp/30333/ws" \
  --telemetry-url "wss://telemetry.polkadot.io/submit 0" \
  -- \
  --execution wasm \
  --chain kusama \
  --database=RocksDb \
  --unsafe-pruning \
  --pruning=1000 \
  --port 30343

Restart=always
RestartSec=120
[Install]
WantedBy=multi-user.target
```

Then ctrl + s then ctrl + x to save & exit that file.

Let's start the collator:

```shell
sudo systemctl daemon-reload && sudo systemctl enable tinkernet-collator && sudo systemctl start tinkernet-collator.service
```

Now, let's check that the chain is running

```shell
sudo systemctl status tinkernet-collator.service
```

If the service indicates it's "running" and you see no errors, you should be ok. If not, you can debug using one of the following:
```shell
sudo journalctl -fu tinkernet-collator
sudo systemctl status --full --lines=100 tinkernet-collator
```

Syncing the Kusama relaychain will take a long time, depending on your download speed (it needs to download something like 130 gb via P2P). If you'd like to accelerate that process you can download a snapshot of the Kusama relaychain to start with:

```shell
sudo systemctl stop tinkernet-collator.service
```

Run `ls /var/lib/tinkernet` and you should now see that "chains" and "polkadot" directories have been created.

To accelerate setup, delete the relay chain state and download a snapshot instead:

```shell
sudo apt -y install curl lz4 tar
sudo rm -rf /var/lib/tinkernet/polkadot/chains/ksmcc3/*
sudo curl -o - -L https://ksm-rocksdb.polkashots.io/snapshot | sudo lz4 -c -d - | sudo tar -x -C /var/lib/tinkernet/polkadot/chains/ksmcc3
```

Now we can start the the service again:

```shell
sudo systemctl start tinkernet-collator.service
```

Using your browser, check on the "telemetry" website to see if your node is online:
`https://telemetry.polkadot.io/#list/0xd42e9606a995dfe433dc7955dc2a70f495f350f373daa200098ae84437816ad2`


## Set session keys

Now we need to rotate keys and set our keys on chain to associate our on-chain acct with the collator node software in order to join the active set of collators and receive rewards. Ensure the collator is running or this step won't work.

```shell
sudo curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "author_rotateKeys", "params":[]}' http://127.0.0.1:9933/
```

You will be greeted with an output that looks like:

```shell
{"jsonrpc":"2.0","result":"0xprivate_key_will_be_here0","id":1}
```

"result":"**0x_private_key_will_be_here0**" is what we are interested in.

You need to make sure that you have a Polkadot/Substrate account set up, here's some videos in case you don't know how to do that:

1. Polkadot JS Videos  https://www.youtube.com/watch?v=dG0DP9vayPY    https://www.youtube.com/watch?v=BpTQBAyFvEk

2. Talisman Video   https://docs.talisman.xyz/talisman/talisman-initiation/setup-a-talisman-wallet  

Now that you have made an account using one of those extensions, head on over to the InvArch Tinkernet section of Polkadot JS:

https://polkadot.js.org/apps/?rpc=wss%3A%2F%2Ftinker.invarch.network#/extrinsics

1. Ensure that you are in the Developer tab (the top header), under Extrinsics->Submission.
2. In the `using the selected account` field, select the account you just made for the collator.
3. In the `submit the following extrinsic` field, select `session`.
4. In the next field (to the right), select `setKeys(keys, proof)`.
5. In the `keys:` field, paste in your **0x_private_key_will_be_here0** from your node.
6. In the `proof` field, type in `0`.
7. Submit the transaction.


## Register as a collator candidate (to join the active set)
### Join Candidates
1. Return to the Extrinics page: https://polkadot.js.org/apps/?rpc=wss%3A%2F%2Ftinker.invarch.network#/extrinsics
2. In the `using the selected account` field, select the account you just made for the collator.
3. In the `submit the following extrinsic` field, select `collatorSelection`.
4. In the next field (to the right), select `registerAsCandidate()`.
5. Submit the transaction.
6. Wait 2 rounds (roughly 12 hours) for your collator registration to take effect.

Congratulations, you should now be onboarded as a collator!


## Prologue: Node Monitoring

As a collator node operator, you should also set up monitoring for your node, including Prometheus, Grafana, and AlertManager. Good instructions for that setup can already be found at:
* Prometheus/Grafana (HDX): https://docs.hydradx.io/node_monitoring
* AlertManager: https://wiki.polkadot.network/docs/maintain-guides-how-to-monitor-your-node

## Updating your Node

Once there are new releases, a quick process to update your node is:
```
sudo systemctl stop tinkernet-collator
sudo wget -O /var/lib/tinkernet/tinker-raw.json https://github.com/Abstracted-Labs/InvArch/releases/latest/download/tinker-raw.json
sudo chown tinkernet-collator:tinkernet-collator /var/lib/tinkernet/tinker-raw.json
sudo wget -O /usr/local/bin/invarch-collator https://github.com/Abstracted-Labs/InvArch/releases/latest/download/invarch-collator
sudo chmod +x /usr/local/bin/invarch-collator
sudo chown tinkernet-collator:tinkernet-collator /usr/local/bin/invarch-collator
sudo systemctl start tinkernet-collator
sudo journalctl -fu tinkernet-collator
```
