# Milkman Bot Guide

This is a guide for those who wish to run a Milkman bot. We will assume that you
have access to a Kubernetes cluster. If you don't already have one, I recommend
GKE. You can get a single-node cluster with E2 micro for < $5 / month.

Once you've connected your kubernetes CLI to your cluster, you first need to setup
the infura api key secret. To do so, edit [infura-api-key-secret.yaml](./infura-api-key-secret.yaml), 
replacing "# add your API key here" with your API key. Then, you can create the
secret by running the following:

```bash
$ kubectl apply -f infura-api-key-secret.yaml
```

Once you've created the secret, running the bot is as simple as running the following
command:

```bash
$ kubectl apply -f milkman-bot-deployment.yaml
```

That's it! Or at least that's the basic stuff, if you want to use the sensible defaults.

## Configuration

You can also configure the bot with environment variables via milkman-bot-deployment.yaml.
The following are optional parameters.

### MILKMAN_NETWORK

*Default:* 
`mainnet`

*Description:*
Useful if you want to run integration tests on a testnet. For example, you could
pass in 'goerli'.
            
### MILKMAN_ADDRESS

*Default:*
0x11C76AD590ABDFFCD980afEC9ad951B160F02797

*Description:*
Address of the core milkman contract that the bot watches.

### STARTING_BLOCK_NUMBER

*Default:*
Current block number

*Description:*
For if, for some reason, existing bots didn't pick up a historical requested swap
and you need to pick it up.

### POLLING_FREQUENCY_SECS

*Default:*
`10`

*Description:*
The bot check every `POLLING_FREQUENCY_SECS` seconds for new swaps. You might
want to raise this if you're worried about hitting your limit, but I've never had
an issue.

### RUST_LOG

*Default:*
`INFO`

*Description:*
Controls level of logging. You can use `DEBUG` for more logs.

### HASH_HELPER_ADDRESS

*Default:*
0x49Fc95c908902Cf48f5F26ed5ADE284de3b55197

*Description:*
Self-explanatory.


### NODE_BASE_URL

*Default*:
N/A

*Description*:
If you want to use something other than Infura. Needs to be JSON-RPC compatible. 

### SLIPPAGE_TOLERANCE_BPS

*Default*:
50

*Description*:
The slippage tolerance that is set on the orders the bot places (compared to the quoted amount). Reducing this may make a price checker that is "just" not passing accept the order, however it may make it more difficult for solvers to settle.
