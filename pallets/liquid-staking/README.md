# Liquid Staking Pallet
- Liquid staking pallet tightly couples with pallet-staking.
- It is using orml MultiCurrency trait and module (pallet) for supporting multiple assets.  
- The pallet code is at `pallets/liquid-staking`

## Running the tests
- `cargo test -p pallet-liquid-staking`

Note: The node is not compiling.
## Rubric
- [x] New stakers can directly stake through this pallet, which controls all the staked dot, and generates a derivative token as well.
- [x] Simple voting system in the pallet where holders of the derivative token can influence which validators the pallet backs.
- [x] The derivative token is transferrable, hence “liquid” staking.
- [x] Reward and slashing is accurate and reliable across this multi-pallet system.
- [x] Users can vote on referenda using the tokens which are staked and managed by this pallet.

## Notes
- The liquid token is generated at initial 10:1 ratio. For 1 staked currency, you get back 10 liquid currency. This is probably bad for democracy since it does not take into account the currency weights. Needs more nuanced solution to address it. 
- The voting system is super naive where out of all validators, top 2 are selected in the nomination pool controlled by the pallet. It also does iterating and sorting on the list which is fine as long as maximum validators supported is a low number. 
- Reward and Slashing are accurate since as the rewards/slash are generated, the total staked currency pool increases/decreases. When the user exchanges liquid currency back to staked currency, the ratio increases/decreases depending on the pool size. Eg. User staked 1 DOT, got back 10 LDOT. Now the rewards doubled the stake pool and the stake pool size is 2 DOT. When user tries to get back their dot, they will receive (10*2/10 = 2) DOT back.
- Users can vote on democracy referenda via both liquid or staking currency. One drawback is both liquid and staking currency has same weight. I created a new call `vote_v2` just so I don't have to deal with too many breaking apis. If you look at democracy-pallet code (`pallets/democracy/lib.rs`) it accepts a MultiCurrency in its configuration. There are couple of tests to vote on existing referenda via both liquid and staking currency in `pallets/liquid-staking/tests.rs` to verify this.

