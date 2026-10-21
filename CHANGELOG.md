# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.1](https://github.com/memcastle/memcastle/compare/0.5.0..0.5.1) - 2026-10-21

### 🐛 Bug Fixes

- **ci** Smoke-test current Core assets in AUR ([#353](https://github.com/memcastle/memcastle/issues/353)) - ([c7ef423](https://github.com/memcastle/memcastle/commit/c7ef4235c3b27c98b86c639a96fd80ab3e6d7641))
- **ci** Check draft release with publishing app ([#352](https://github.com/memcastle/memcastle/issues/352)) - ([75fc969](https://github.com/memcastle/memcastle/commit/75fc969ca2b7eada6dcac16084ef0781b880a5c3))
- **ci** Grant release recovery attestation permissions ([#351](https://github.com/memcastle/memcastle/issues/351)) - ([f0ce0ce](https://github.com/memcastle/memcastle/commit/f0ce0ceffce651e95c1b0729288b711a59abb8d5))
- **ci** Recover releases with no bundled source archives ([#349](https://github.com/memcastle/memcastle/issues/349)) - ([9f636d1](https://github.com/memcastle/memcastle/commit/9f636d116615b6f728cb2a17ca2b1b06888fcdc3))
- **plugin** Infer versions from SemVer release tags ([#356](https://github.com/memcastle/memcastle/issues/356)) - ([c00b69e](https://github.com/memcastle/memcastle/commit/c00b69edca1ad4d9b0a07677887a4923cf5a4d20))
- **release** Publish skills-only archive ([#354](https://github.com/memcastle/memcastle/issues/354)) - ([e2f5de7](https://github.com/memcastle/memcastle/commit/e2f5de707a30aca46bba5d47825c93aaa8308765))

## [0.5.0](https://github.com/memcastle/memcastle/compare/0.4.1..0.5.0) - 2026-10-20

### 💫 Features

- **auth** Share provider accounts across embedding connections ([#333](https://github.com/memcastle/memcastle/issues/333)) - ([f77338a](https://github.com/memcastle/memcastle/commit/f77338a674688dbc5c1cabef8d4b871bb8b2b814))
- **embed** Make embedding providers discoverable and testable ([#337](https://github.com/memcastle/memcastle/issues/337)) - ([27ac1c1](https://github.com/memcastle/memcastle/commit/27ac1c10c10e98b6a49ff55e31f37dce2e8e9363))
- **embed** Run embedding providers as plugin modules ([#331](https://github.com/memcastle/memcastle/issues/331)) - ([93ce489](https://github.com/memcastle/memcastle/commit/93ce489fe2ceead867a618960c089ae87d6d1f1e))
- **embed** Track embedding spaces and support safe re-embedding - ([23d0b37](https://github.com/memcastle/memcastle/commit/23d0b37f114d84aa45d2d4210a2a8d3f86ae2aec))
- **mining** Add reusable miner presets and source configurations ([#329](https://github.com/memcastle/memcastle/issues/329)) - ([3cc7a72](https://github.com/memcastle/memcastle/commit/3cc7a721957913dd3603f2815ea81d40c8dbffc6))
- **plugin** Validate standalone lifecycle and retire source init ([#347](https://github.com/memcastle/memcastle/issues/347)) - ([88b3564](https://github.com/memcastle/memcastle/commit/88b35645e676412d6154a2d0e58e9c4259ea7e2d))
- **plugin** Accept separate source and integration roots ([#320](https://github.com/memcastle/memcastle/issues/320)) - ([1becd31](https://github.com/memcastle/memcastle/commit/1becd31a8d986c4cdc48434579c6663f04540349))
- **plugin** Derive release archives from the descriptor and Git ([#319](https://github.com/memcastle/memcastle/issues/319)) - ([65ecc3e](https://github.com/memcastle/memcastle/commit/65ecc3e55eba657a63d4e4b9e9256dd8320e796a))
- **plugin** Package and smoke-test plugins locally ([#315](https://github.com/memcastle/memcastle/issues/315)) - ([9dc404a](https://github.com/memcastle/memcastle/commit/9dc404acdf1e326d49c2262bfde81a31222038f3))
- **plugins** Register Slack source ([#336](https://github.com/memcastle/memcastle/issues/336)) - ([76da91d](https://github.com/memcastle/memcastle/commit/76da91d613d8b3bc237161d0c85e494289360ff0))
- **plugins** Publish development Core and share plugin workflows ([#304](https://github.com/memcastle/memcastle/issues/304)) - ([219e7a2](https://github.com/memcastle/memcastle/commit/219e7a20f14242dc5b02eaf42be60b5ea6cea9ff))
- **plugins** Validate standalone plugin workflow against current Core ([#294](https://github.com/memcastle/memcastle/issues/294)) - ([4d1161a](https://github.com/memcastle/memcastle/commit/4d1161af833332ca0b90e82765f03351297fad73))
- **plugins** Distribute independently versioned provider modules ([#285](https://github.com/memcastle/memcastle/issues/285)) - ([ca32ca5](https://github.com/memcastle/memcastle/commit/ca32ca596fc13c7f21ae3bca66cc46e5a3ebb1e7))
- **sources** Grant selected credentials to WASM adapters ([#335](https://github.com/memcastle/memcastle/issues/335)) - ([412542e](https://github.com/memcastle/memcastle/commit/412542e8f589123ff198e5ecc09ebb4d6b98ce54))
- **web** Rework exploration and mining navigation ([#339](https://github.com/memcastle/memcastle/issues/339)) - ([703e1b0](https://github.com/memcastle/memcastle/commit/703e1b0f7036428d98a945f3fa8b1d04646374c2))
- **web** Add resource detail drawers and unified job launcher ([#334](https://github.com/memcastle/memcastle/issues/334)) - ([bbc732b](https://github.com/memcastle/memcastle/commit/bbc732bcb915be8e7aade933ac820b5ca58208f3))
- **web** Make layout fluid and show accessible job progress ([#325](https://github.com/memcastle/memcastle/issues/325)) - ([2158427](https://github.com/memcastle/memcastle/commit/21584272e59bd499af2690bc22ad95de2ac65e57))

### 🐛 Bug Fixes

- **audit** Bound memory while scanning drawers ([#286](https://github.com/memcastle/memcastle/issues/286)) - ([4bafdc4](https://github.com/memcastle/memcastle/commit/4bafdc43f83eab4f097f694e834f3d4fbebd5854))
- **ci** Expose Core binary path to integrations - ([f6ad794](https://github.com/memcastle/memcastle/commit/f6ad794580d84002b7c567d4786cf805a5e732ec))
- **ci** Keep main selector during plugin migration - ([16c76d6](https://github.com/memcastle/memcastle/commit/16c76d6121b47c07e5c1d6f20048de1b99515c35))
- **ci** Retain only the current development Core assets - ([1ee044b](https://github.com/memcastle/memcastle/commit/1ee044b23f16ed91d228e59dfbfbd1053a51d2f9))
- **client** Allow registry timeout for plugin fetches - ([f1be816](https://github.com/memcastle/memcastle/commit/f1be816f178c6579a2aff3b9fe623ee0c4840b28))
- **plugins** Report explicit catalogue search failures - ([35704aa](https://github.com/memcastle/memcastle/commit/35704aaf9b6b0ebde4688b41e4422168cdad8e44))
- **source** Preserve failures after optional credential lookups ([#346](https://github.com/memcastle/memcastle/issues/346)) - ([3ebb400](https://github.com/memcastle/memcastle/commit/3ebb4006cc5b68430c0b9cb5126669d9dbeba404))
- **source** Allow optional selected credentials ([#343](https://github.com/memcastle/memcastle/issues/343)) - ([2da21f2](https://github.com/memcastle/memcastle/commit/2da21f26c8b4352c505127b33079cbb6f27db4e0))
- **source** Pin registry install to previewed archive - ([e59ca6d](https://github.com/memcastle/memcastle/commit/e59ca6d9b7425a6491c444a4aebfcc9811a2f0b0))
- **trigger** Keep running triggers on transient context failure - ([4b70991](https://github.com/memcastle/memcastle/commit/4b70991d98a14ca5871cad140d85c690b42ee44b))
- **trigger** Recover deliveries missed by fast restarts - ([6ac7171](https://github.com/memcastle/memcastle/commit/6ac7171f0f94f4aedbce5de7679278218291b1e6))
- **triggers** Reset due time when a timer plan changes - ([b0a5882](https://github.com/memcastle/memcastle/commit/b0a58826cc35c5cda6d015ca66991b49a8946244))
- **triggers** Wait a fresh interval after re-enabling - ([7c7489b](https://github.com/memcastle/memcastle/commit/7c7489b43716d4ae439929261f2569fb7f7617fe))

### ⚡ Performance

- **ci** Reduce test cache churn and move timed miner check to slow suite ([#338](https://github.com/memcastle/memcastle/issues/338)) - ([43c5308](https://github.com/memcastle/memcastle/commit/43c5308dacb7219c7bd0ea39bf9b23d98a15097c))
- **tasks** Skip unchanged web and integration builds ([#310](https://github.com/memcastle/memcastle/issues/310)) - ([5e534ab](https://github.com/memcastle/memcastle/commit/5e534abd3edcfeb48ab136248613f70b1aeaa3a3))

### 🔨 Refactor

- **ci** Remove migrated main Core selector - ([082aa87](https://github.com/memcastle/memcastle/commit/082aa87c4219fc56dace583b7df1cbb8ebe415f2))
- **claude** Distribute Claude modules through Anthropic plugin ([#311](https://github.com/memcastle/memcastle/issues/311)) - ([29ea211](https://github.com/memcastle/memcastle/commit/29ea2118c5383cb1cd84a59a2b105d381a7da539))
- **github** Extract mining source into standalone plugin ([#313](https://github.com/memcastle/memcastle/issues/313)) - ([0fca75f](https://github.com/memcastle/memcastle/commit/0fca75fa9d6ab36111e52898255374f4fafe584c))
- **openai** Extract Codex and ChatGPT product modules ([#312](https://github.com/memcastle/memcastle/issues/312)) - ([2cd5414](https://github.com/memcastle/memcastle/commit/2cd541449a0a7e46dfb24aef293d4c18ec2cedb3))
- **opencode** Extract source and integration into standalone plugin ([#296](https://github.com/memcastle/memcastle/issues/296)) - ([89bf9fd](https://github.com/memcastle/memcastle/commit/89bf9fde466fe81f2d8d22afee542c37af032120))
- **pi** Extract Pi modules into standalone plugin ([#295](https://github.com/memcastle/memcastle/issues/295)) - ([6ed689c](https://github.com/memcastle/memcastle/commit/6ed689cfb595eaa8d23ec3a0245626f452559624))
- **tests** Separate slow checks and reduce source test setup ([#303](https://github.com/memcastle/memcastle/issues/303)) - ([a030e30](https://github.com/memcastle/memcastle/commit/a030e308bfafb2590711a6ef6af3d3849e1a6488))

### 📚 Documentation

- **api** Describe dashboard authentication exception - ([f3884e3](https://github.com/memcastle/memcastle/commit/f3884e3dbbe86c731cab0e381a7c259f6f1e0b19))
- **api** Explain source config breadth approval - ([a4e6b0c](https://github.com/memcastle/memcastle/commit/a4e6b0c7ad15a70288cb1d2f77991befe2e6c95d))
- **api** List all miner update fields - ([3b5bf0a](https://github.com/memcastle/memcastle/commit/3b5bf0a7773c3992c3e4a59d017f1e90d65c17bc))
- **api** List profile for GET search and recall - ([3af2601](https://github.com/memcastle/memcastle/commit/3af26019dba72c4352788fc8629c48868d59536a))
- **api** Document all source origins - ([c790779](https://github.com/memcastle/memcastle/commit/c790779192913d7cf991b16de3f50f23c7e2e93e))
- **assets** Separate Core and provider assets - ([28d54fd](https://github.com/memcastle/memcastle/commit/28d54fd90aa8b726e31b0d9eed4c54d240efa056))
- **auth** Wrap credential backend invariant - ([bb41c8a](https://github.com/memcastle/memcastle/commit/bb41c8a3bdc64d377d20f2cc6d3fdcd477cd715e))
- **auth** Clarify where credentials are persisted - ([c45484d](https://github.com/memcastle/memcastle/commit/c45484d32f908e567abbfc7996f934a19ed76d4b))
- **auth** Describe public dashboard assets accurately - ([a1250aa](https://github.com/memcastle/memcastle/commit/a1250aaf84cd6b97f9e726a41a8f5f64df0f0c87))
- **cli** Distinguish embed job from provider test - ([6da0ca2](https://github.com/memcastle/memcastle/commit/6da0ca2d3b512eea976a5e9149c4b26d3f5fb74e))
- **cli** Show source config credential flags - ([00a02ac](https://github.com/memcastle/memcastle/commit/00a02acfd3ebb06bad44cd8891bda83b2f94dc24))
- **cli** Remove retired source init instructions - ([70d600d](https://github.com/memcastle/memcastle/commit/70d600d2bc0a45161aed9084ebb7da73e70410ae))
- **cli** List local tooling exceptions - ([9c27eac](https://github.com/memcastle/memcastle/commit/9c27eacf6ca0908ba5a364ce6170778bd190a842))
- **install** Describe current Core package contents - ([3f3def0](https://github.com/memcastle/memcastle/commit/3f3def0347f40a62c0fd418a0d2d48590b835d28))
- **integration** Describe offline version check - ([8432d72](https://github.com/memcastle/memcastle/commit/8432d726fc4b4ffd74ba0dad9dd0edf6be4c60ba))
- **integration** Keep install command readable - ([41cf9ce](https://github.com/memcastle/memcastle/commit/41cf9cec51fac8efc63d813b5cda52ea47e7dc08))
- **integration** Distinguish plugin-capable Core releases - ([18dd1b9](https://github.com/memcastle/memcastle/commit/18dd1b9e324283a68afde71f92ef3f2a735d9379))
- **integrations** Describe shared-skills-only Core tasks - ([6e441e8](https://github.com/memcastle/memcastle/commit/6e441e835f86c731738618670949a4747017b6e9))
- **integrations** Distinguish full and limited lifecycle clients - ([73f57da](https://github.com/memcastle/memcastle/commit/73f57da233feda34ed2f7043b4ce5d50c135c26b))
- **mcp** Avoid stale tool count - ([a3553cc](https://github.com/memcastle/memcastle/commit/a3553ccbc9a51faca3f80d4eef8801a914a501c8))
- **quickstart** Describe directory chunking - ([23fc9d9](https://github.com/memcastle/memcastle/commit/23fc9d9b5ab959bf0dda1e4ab64bfaeaff7ddc6e))
- **skills** Clone the installed release tag - ([4a0360f](https://github.com/memcastle/memcastle/commit/4a0360fa7bb16099bc71c5fac25ed855f44bf12e))
- **skills** Attribute skills to provider integrations - ([933c7c2](https://github.com/memcastle/memcastle/commit/933c7c2e845ab5c3240f0313e01e4ed1f35c584c))
- **source** Require consent for every permission-set change - ([e53e704](https://github.com/memcastle/memcastle/commit/e53e704ffb2fd32fa2e7be6a6a9b257f8994ec1e))
- **source** Correct bundled update example - ([e089c21](https://github.com/memcastle/memcastle/commit/e089c21a91dfb284025e097a00c1a6c309232530))
- **source** Identify ChatGPT provider plugin - ([056def9](https://github.com/memcastle/memcastle/commit/056def9935d2cba560f6c84fca92028a9c37b869))
- **sources** Correct legacy registry and Codex examples - ([73fb0dc](https://github.com/memcastle/memcastle/commit/73fb0dcf74a5df58462dad2eeb38d06df44fa8c3))
- **sources** Use current release line in manifest example - ([f756b23](https://github.com/memcastle/memcastle/commit/f756b23d181eed05173da17cc6aed5a0fa15830a))
- **storage** Scope memory read limits - ([672aa7c](https://github.com/memcastle/memcastle/commit/672aa7cbb965403097afe148779bd5e3c985c0c8))
- **triggers** Use a daemon-readable webhook secret - ([0bad6a8](https://github.com/memcastle/memcastle/commit/0bad6a8df9d7b66bc50b73dd17cdd4559ad001b9))

### 🧪 Tests

- **sources** Use mock component for generic Core tests ([#306](https://github.com/memcastle/memcastle/issues/306)) - ([dcdee0f](https://github.com/memcastle/memcastle/commit/dcdee0fb442ecfcdaf58651673f510464281cefe))
- **web** Allow lazy navigation under CI load ([#308](https://github.com/memcastle/memcastle/issues/308)) - ([12ad575](https://github.com/memcastle/memcastle/commit/12ad5757734a4a27e3c6509c41d401d7dd77d5fe))

### 🏗️ Build

- **deps** Bump the web-dependencies group across 1 directory with 6 updates ([#316](https://github.com/memcastle/memcastle/issues/316)) - ([a7bee51](https://github.com/memcastle/memcastle/commit/a7bee51c6219486f0dc56a4ee8db5bf70aa80d07))
- **deps** Bump the rust-dependencies group across 1 directory with 6 updates ([#318](https://github.com/memcastle/memcastle/issues/318)) - ([c8fe1e1](https://github.com/memcastle/memcastle/commit/c8fe1e14539d93db1d0757b6ea0abbc3950225da))
- **deps** Bump surrealdb-core from 3.3.0 to 3.3.2 in the rust-dependencies group ([#302](https://github.com/memcastle/memcastle/issues/302)) - ([0e15482](https://github.com/memcastle/memcastle/commit/0e15482ad1f2f7fd3c2b041cb13c7cf1c6231a89))
- **deps** Bump the web-dependencies group in /web with 4 updates ([#299](https://github.com/memcastle/memcastle/issues/299)) - ([6aab88c](https://github.com/memcastle/memcastle/commit/6aab88c5f11792c6c13e3e7d4f59d907de113aa4))
- **deps** Bump the rust-dependencies group with 2 updates ([#301](https://github.com/memcastle/memcastle/issues/301)) - ([c01b986](https://github.com/memcastle/memcastle/commit/c01b98665363e2365ac52d9f9b6b8626b8dd3e9e))
- **deps** Bump actions/checkout from 4 to 7 in the actions group ([#300](https://github.com/memcastle/memcastle/issues/300)) - ([15cf4bf](https://github.com/memcastle/memcastle/commit/15cf4bf9af6e23af23fb5868c1c051b75c5bdb68))
- **deps-dev** Bump happy-dom from 20.14.6 to 21.0.1 in /web ([#317](https://github.com/memcastle/memcastle/issues/317)) - ([6f690cd](https://github.com/memcastle/memcastle/commit/6f690cd387c60b6e0b93795d6df4bc9c3a102c24))

### 🔧 CI

- **plugins** Publish only Linux amd64 development Core ([#307](https://github.com/memcastle/memcastle/issues/307)) - ([51b6ba7](https://github.com/memcastle/memcastle/commit/51b6ba7958dd4aa537a4a17419f0f468f27ea613))
- Migrate architecture guards to ast-grep ([#323](https://github.com/memcastle/memcastle/issues/323)) - ([e582210](https://github.com/memcastle/memcastle/commit/e582210bada8b30114b064d2a2be3736658cc9b1))
- Move Clippy from pre-commit to CI ([#322](https://github.com/memcastle/memcastle/issues/322)) - ([8c34b3c](https://github.com/memcastle/memcastle/commit/8c34b3c976b8ad127e6c936ecd70115825cdf8b5))
- Test in-tree fixtures without provider discovery ([#321](https://github.com/memcastle/memcastle/issues/321)) - ([9b54aef](https://github.com/memcastle/memcastle/commit/9b54aef22ab8eef1ea2882bf213ca2586b3c9a0b))
- Restore Codecov PR reports and run all workflow checks ([#309](https://github.com/memcastle/memcastle/issues/309)) - ([4c212b2](https://github.com/memcastle/memcastle/commit/4c212b2ec587ba1450e4a1e13d8dca688a499f81))

## [0.4.1](https://github.com/memcastle/memcastle/compare/0.4.0..0.4.1) - 2026-10-10

### 🐛 Bug Fixes

- **registry** Prepare organization site and registry migration (#267 / #283) - ([144a4ce](https://github.com/memcastle/memcastle/commit/144a4ce0c4774e506ff62a7ee32e588447ef4da1))

### 🏗️ Build

- **deps** Bump the opencode-dependencies group in /integrations/opencode with 3 updates ([#280](https://github.com/memcastle/memcastle/issues/280)) - ([708cac3](https://github.com/memcastle/memcastle/commit/708cac3fbdc2acf98177e651df35b2bcb689eee3))
- **deps** Bump rmcp from 3.5.0 to 3.5.1 in the rust-dependencies group ([#281](https://github.com/memcastle/memcastle/issues/281)) - ([d23edfa](https://github.com/memcastle/memcastle/commit/d23edfa56ce7b47a9fdb798b4eaf1b25750101aa))
- **deps** Bump ratatui from 0.29.0 to 0.30.2 ([#282](https://github.com/memcastle/memcastle/issues/282)) - ([a03b2c2](https://github.com/memcastle/memcastle/commit/a03b2c233c424c84fc2a6374e5a49eab548c9072))
- **deps-dev** Bump @earendil-works/pi-coding-agent from 1.0.2 to 1.0.4 in /integrations/pi in the pi-dependencies group across 1 directory ([#278](https://github.com/memcastle/memcastle/issues/278)) - ([c29747f](https://github.com/memcastle/memcastle/commit/c29747fc845b8c138bbe90d2f89e63b861997c8f))
- **deps-dev** Bump vite from 8.3.2 to 8.3.3 in /web in the web-dependencies group ([#279](https://github.com/memcastle/memcastle/issues/279)) - ([9d77a81](https://github.com/memcastle/memcastle/commit/9d77a81645740d4d048f2e40a7ede4e48332a51c))

## [0.4.0](https://github.com/memcastle/memcastle/compare/0.3.0..0.4.0) - 2026-10-09

### 💫 Features

- **chatgpt** Add export and OAuth-backed project mining ([#249](https://github.com/memcastle/memcastle/issues/249)) - ([eccaae4](https://github.com/memcastle/memcastle/commit/eccaae4250ab2ff3cb266102dbb683778fd0c840))
- **claude** Add Claude Code integration and history source ([#245](https://github.com/memcastle/memcastle/issues/245)) - ([fb5e172](https://github.com/memcastle/memcastle/commit/fb5e1720647978f200be59bde76239a2e71ac1ae))
- **cli** Highlight JSON on terminals and honor FORCE_COLOR ([#263](https://github.com/memcastle/memcastle/issues/263)) - ([ee4f02c](https://github.com/memcastle/memcastle/commit/ee4f02c43e9d48b79d99fd2e6bd0445a9a0e6ed6))
- **cli**  🚨 **breaking** Print a readable result in a terminal and JSON in a pipe, for every command ([#225](https://github.com/memcastle/memcastle/issues/225)) - ([8a0ed70](https://github.com/memcastle/memcastle/commit/8a0ed70f3c1acd7ecd7bb025b5849bb2602914fc))
- **codex** Add Codex integration and history source ([#248](https://github.com/memcastle/memcastle/issues/248)) - ([7ed0fca](https://github.com/memcastle/memcastle/commit/7ed0fcaf20f0b03f4127d3b4ec75f984a3d90081))
- **doctor** Add read-only configuration and runtime diagnostics ([#258](https://github.com/memcastle/memcastle/issues/258)) - ([3cc1d2f](https://github.com/memcastle/memcastle/commit/3cc1d2f786f016e0d5e9e0be094572a64a0fed8f))
- **github** Add scoped GitHub mining source ([#261](https://github.com/memcastle/memcastle/issues/261)) - ([820a51a](https://github.com/memcastle/memcastle/commit/820a51a4f71bf5a7ba2c889c2ce85e339ad657fd))
- **integration**  🚨 **breaking** Require Pi 1.0 and use Pi's own MCP client in the Pi integration - ([e41acc1](https://github.com/memcastle/memcastle/commit/e41acc13f92459f290dbcae90557098ae8a63d0c))
- **integration** Add a confirmed palace audit and repair command to Pi and OpenCode ([#232](https://github.com/memcastle/memcastle/issues/232)) - ([79a4416](https://github.com/memcastle/memcastle/commit/79a44163c39ae801244a416fc1b7d43a762d84b7))
- **integration**  🚨 **breaking** Declare and package skills with each agent integration ([#231](https://github.com/memcastle/memcastle/issues/231)) - ([d195b81](https://github.com/memcastle/memcastle/commit/d195b817ecb0593c30e8d86181aaf4313944debd))
- **memory** Add configurable source preferences ([#262](https://github.com/memcastle/memcastle/issues/262)) - ([6558c99](https://github.com/memcastle/memcastle/commit/6558c99b3e1d7cc3ac577c9f1788c21ba57258f9))
- **memory** Add auditable fact lifecycle and conflict handling ([#257](https://github.com/memcastle/memcastle/issues/257)) - ([efdea0a](https://github.com/memcastle/memcastle/commit/efdea0a1a86d6c2237ba052262e95d685511cb4a))
- **miners** Add persistent miner configuration with CLI and read-only MCP ([#223](https://github.com/memcastle/memcastle/issues/223)) - ([b9911d8](https://github.com/memcastle/memcastle/commit/b9911d8a9b4201bc1db5e21a4fbc0bdbb191b8eb))
- **mining**  🚨 **breaking** Mine <source> [place] [key=value]... with per-source options ([#230](https://github.com/memcastle/memcastle/issues/230)) - ([5d79318](https://github.com/memcastle/memcastle/commit/5d79318f1b4a489101012eed11429eda86539ad8))
- **mining**  🚨 **breaking** Install bundled sources from the start and publish the official registry ([#227](https://github.com/memcastle/memcastle/issues/227)) - ([e3f0ad3](https://github.com/memcastle/memcastle/commit/e3f0ad310be80bac8dc7dd9d5ace5ce1a1c0f1f5))
- **mining**  🚨 **breaking** Sign mining sources in with OAuth and hand them a fresh token ([#228](https://github.com/memcastle/memcastle/issues/228)) - ([f6d34a9](https://github.com/memcastle/memcastle/commit/f6d34a99193fb59cf2cf8938efb16a663af411f2))
- **trigger**  🚨 **breaking** Add opt-in source triggers for asynchronous mining ([#241](https://github.com/memcastle/memcastle/issues/241)) - ([f69f0b6](https://github.com/memcastle/memcastle/commit/f69f0b6163dc2b7505ea883292a99e126c349096))
- **tui** Add live terminal operations console ([#259](https://github.com/memcastle/memcastle/issues/259)) - ([51bee30](https://github.com/memcastle/memcastle/commit/51bee303c0872dece62b3c3635e5d9ff8568273e))
- **web** Push updates to the dashboard over server-sent events ([#229](https://github.com/memcastle/memcastle/issues/229)) - ([b0ce74d](https://github.com/memcastle/memcastle/commit/b0ce74d11b5fdb3bf24afce10d7658047788d6c5))
- **web** Add an opt-in web dashboard served under /ui ([#217](https://github.com/memcastle/memcastle/issues/217)) - ([af6ff96](https://github.com/memcastle/memcastle/commit/af6ff96a63a0167ef6d83c323ca12de4bbf430e2))

### 🐛 Bug Fixes

- **cli** Limit named source update checks to the requested source - ([819c583](https://github.com/memcastle/memcastle/commit/819c58356cc94797116679c1aba46c6c0771baa9))
- **cli** Honor session mode for miner and trigger commands - ([71ad298](https://github.com/memcastle/memcastle/commit/71ad298ccce53838b31511bb37b85525e393dc60))
- **jobs** Keep API and MCP responsive during mining ([#260](https://github.com/memcastle/memcastle/issues/260)) - ([60d4638](https://github.com/memcastle/memcastle/commit/60d463829fa43611f41813ee0251c5f5603bcee8))
- **mining** Capture program output through files so a CLI that truncates pipes answers whole ([#224](https://github.com/memcastle/memcastle/issues/224)) - ([e700a2e](https://github.com/memcastle/memcastle/commit/e700a2e293c7f8acdbeee595d8e773d9d64f7495))
- **source** Honor Claude RFC 3339 since cutoff - ([f4259b9](https://github.com/memcastle/memcastle/commit/f4259b944f45a4a3201f4f79229dfd1a4091359a))
- **web** Render duplicates from the REST response envelope - ([25342f3](https://github.com/memcastle/memcastle/commit/25342f369f9de1e2301ff2fd3c2db2669e798030))

### 🔨 Refactor

- **miner**  🚨 **breaking** Unify source options and allow per-run overrides ([#264](https://github.com/memcastle/memcastle/issues/264)) - ([a1b243d](https://github.com/memcastle/memcastle/commit/a1b243dee7b4774be9a15e0f6bda3540e7577a05))

### 📚 Documentation

- **adr** Standardize status sections in recent decisions - ([dbf8399](https://github.com/memcastle/memcastle/commit/dbf839942ff048e533ea2ec68b174bd27331595a))
- **api** Clarify remote datastore location in config report - ([d339f01](https://github.com/memcastle/memcastle/commit/d339f0181fcdffc4c81268b0951c573b2e286fcd))
- **architecture** Account for ChatGPT web OAuth - ([16b2027](https://github.com/memcastle/memcastle/commit/16b2027419c340fd5865fdf4886df8a525bd444e))
- **claude** Add Claude Code to documentation sidebar ([#246](https://github.com/memcastle/memcastle/issues/246)) - ([d9c94c3](https://github.com/memcastle/memcastle/commit/d9c94c3a588bb74d62e71130fc9a382073a20696))
- **cli** Correct drawer command output description - ([ca6e90c](https://github.com/memcastle/memcastle/commit/ca6e90c7e9da405e1c80d7727642b3923b4867d8))
- **cli** Distinguish source install and update consent flags - ([3e04ad2](https://github.com/memcastle/memcastle/commit/3e04ad235c2d7c4393be25faf438ec925d91df3f))
- **integration** List all shipped agent integrations - ([a2f194b](https://github.com/memcastle/memcastle/commit/a2f194b3aa8183272b0328c827c277e93db3c5f3))
- **mcp** List read-only trigger tool annotations - ([ea6fbd3](https://github.com/memcastle/memcastle/commit/ea6fbd3cee3912d635a9b4ccb97cc3e52bfb40f2))
- **quickstart** Render daemon controls as instructions - ([61c3683](https://github.com/memcastle/memcastle/commit/61c3683d7e0620b7774454febbf01361d70a850d))
- **source** Clarify which origins can be updated - ([2ecd750](https://github.com/memcastle/memcastle/commit/2ecd750da77ec1e0ccc89a6108e677b181401f48))
- **source** Describe unpacked bundle without an index - ([4cabbf2](https://github.com/memcastle/memcastle/commit/4cabbf2586485b161b915a8e2132914931c138c4))
- **source** Align bundled source lists with release packaging - ([0f6cc08](https://github.com/memcastle/memcastle/commit/0f6cc08c5476fb46acd1b3b9f7649ec686d4cf66))

### 🧪 Tests

- **db** Wait for audit before daemon shutdown ([#244](https://github.com/memcastle/memcastle/issues/244)) - ([5deff26](https://github.com/memcastle/memcastle/commit/5deff26868303abeff0e1f59560f14be81156132))
- **retrieval** Add a reproducible retrieval evaluation and benchmark suite ([#222](https://github.com/memcastle/memcastle/issues/222)) - ([447b401](https://github.com/memcastle/memcastle/commit/447b4016b0ecae4d63e61ab41257b873b761969c))
- **trigger** Fix flaky trigger tests and silence the macOS linker warning ([#242](https://github.com/memcastle/memcastle/issues/242)) - ([b71f03e](https://github.com/memcastle/memcastle/commit/b71f03e5a76ac3fa841b30d34455c57d02257090))
- **trigger** Stop two trigger tests from racing the scheduler and the storage lock - ([5770efc](https://github.com/memcastle/memcastle/commit/5770efc8ecf3590e4550c2857190e97e87e01e94))

### 🎨 Style

- **cli** Format mode forwarding handlers - ([f9bb7ab](https://github.com/memcastle/memcastle/commit/f9bb7ab65598c98afd7898fbfb2c8670ec890990))

### 🏗️ Build

- **deps** Bump the rust-dependencies group with 2 updates ([#240](https://github.com/memcastle/memcastle/issues/240)) - ([7c78624](https://github.com/memcastle/memcastle/commit/7c78624c9d590e6c55ec36a10f57bd535702cde2))
- **deps** Bump the rust-dependencies group with 2 updates ([#219](https://github.com/memcastle/memcastle/issues/219)) - ([44b72f6](https://github.com/memcastle/memcastle/commit/44b72f6a4e4bf29682d51ac508f79f3ad75c31d7))
- **deps** Bump actions/setup-node from 4 to 7 in the actions group ([#220](https://github.com/memcastle/memcastle/issues/220)) - ([e1e5db3](https://github.com/memcastle/memcastle/commit/e1e5db322a9e223ac31f451a5bc872b07484ca1b))
- **deps** Bump base64 from 0.22.1 to 0.23.1 ([#221](https://github.com/memcastle/memcastle/issues/221)) - ([9627846](https://github.com/memcastle/memcastle/commit/9627846804c52f5ee74f77f872efaaa46a84d60d))
- **deps-dev** Bump @types/node from 24.19.1 to 26.6.4 in /web ([#238](https://github.com/memcastle/memcastle/issues/238)) - ([2e3e349](https://github.com/memcastle/memcastle/commit/2e3e349bf4e82cdc498f347e2b6ac6187f1852c2))
- **deps-dev** Bump @earendil-works/pi-coding-agent - ([bc7392f](https://github.com/memcastle/memcastle/commit/bc7392fa72b66dc20b4f48980147cfd0e06b384c))
- Allow the linker_messages lint for macOS's __eh_frame warning - ([7a8f348](https://github.com/memcastle/memcastle/commit/7a8f348bf5f01a259020ba18127bcf5d73934033))

### 🔧 CI

- **release** Trial mode for Publish Release, and thin LTO for the release profile ([#226](https://github.com/memcastle/memcastle/issues/226)) - ([f837c92](https://github.com/memcastle/memcastle/commit/f837c92347a73ff417183355c37fccfdea39b502))
- Stabilize Codecov component coverage statuses ([#243](https://github.com/memcastle/memcastle/issues/243)) - ([ecbd83c](https://github.com/memcastle/memcastle/commit/ecbd83c70d7b1e3f74b6087ba0a00f80c3f3ab7b))
- Report TypeScript coverage and add Codecov components ([#233](https://github.com/memcastle/memcastle/issues/233)) - ([9f572fe](https://github.com/memcastle/memcastle/commit/9f572fe61c0daf0ae555f2808513ebc916401064))

## [0.3.0](https://github.com/memcastle/memcastle/compare/0.2.0..0.3.0) - 2026-10-05

### 💫 Features

- **audit**  🚨 **breaking** Name the audit job's wing `wing`, not `scope` - ([64c074c](https://github.com/memcastle/memcastle/commit/64c074c32539f3cae696a9fe41d467c4cac5f8c6))
- **cli** Add `memcastle note` to capture a thought under the current project ([#193](https://github.com/memcastle/memcastle/issues/193)) - ([4e583a5](https://github.com/memcastle/memcastle/commit/4e583a57702e76c1307f18c56888ad925906492e))
- **dedup** Deduplicate memories and resolve entity variants ([#179](https://github.com/memcastle/memcastle/issues/179)) - ([e019943](https://github.com/memcastle/memcastle/commit/e01994342cdf1b25d28defb88da412532d8da451))
- **extract** Extract entities and relationships from mined content ([#156](https://github.com/memcastle/memcastle/issues/156)) - ([8a030f3](https://github.com/memcastle/memcastle/commit/8a030f325c8476279e6c547daea6c10f1321882f))
- **integration** Add memcastle integration list|install|update|remove - ([13151d0](https://github.com/memcastle/memcastle/commit/13151d063f6c98e69849ebb9cc493f92a2ebacbf))
- **integrations** Bundle Pi and OpenCode and ship them with releases - ([74b13b6](https://github.com/memcastle/memcastle/commit/74b13b6b23dbc901012e69eb5eb34ab94bb2491d))
- **integrations** Show classified failures to the user in Pi and OpenCode ([#199](https://github.com/memcastle/memcastle/issues/199)) - ([a653d10](https://github.com/memcastle/memcastle/commit/a653d10c7c5f87ff7fc3358e7c2bb1cdc3b9ba14))
- **integrations** Scope Pi and OpenCode memory to the project's wing and room - ([fa52437](https://github.com/memcastle/memcastle/commit/fa524373ddaae2f43905ed37716eb0c0ba851ad3))
- **integrations** Prove memory modes across Pi and OpenCode - ([86e0fa7](https://github.com/memcastle/memcastle/commit/86e0fa78d8230c8146574973ca8e76b3d1a1f46c))
- **mcp** Annotate every tool with a title and the four behaviour hints ([#205](https://github.com/memcastle/memcastle/issues/205)) - ([8facbf7](https://github.com/memcastle/memcastle/commit/8facbf75819ec514b3fd7210b5041b5da3463950))
- **mining**  🚨 **breaking** Name the source adapter `source`, not `provider` - ([e1266c6](https://github.com/memcastle/memcastle/commit/e1266c684c0852121908a66bb7bbf170c19554c4))
- **mining** Mine a directory into the wing its project file declares - ([17b0b67](https://github.com/memcastle/memcastle/commit/17b0b671dba0bb6ab400f085613b84fafc392d3c))
- **mining**  🚨 **breaking** Pi history is an installed WebAssembly source ([#191](https://github.com/memcastle/memcastle/issues/191)) - ([f302d0e](https://github.com/memcastle/memcastle/commit/f302d0e73b3187d12b794c116c5cd4f581a8b68e))
- **mining** Pluggable source adapters as WebAssembly components ([#178](https://github.com/memcastle/memcastle/issues/178)) - ([db65907](https://github.com/memcastle/memcastle/commit/db659078a4d32ee425a021b97efa19af790e49bd))
- **mining** Unified Source model with incremental mining and a Pi sessions adapter ([#154](https://github.com/memcastle/memcastle/issues/154)) - ([4a7fe28](https://github.com/memcastle/memcastle/commit/4a7fe28aa8d01a16c08b6d76ec2fef17d9c8cde1))
- **opencode** Never attempt a write in a read-only session - ([705e09e](https://github.com/memcastle/memcastle/commit/705e09eff3849bedf4865ea6d22376e33dfc5486))
- **opencode** Checkpoint on an interval, on demand and before compaction - ([c9d931e](https://github.com/memcastle/memcastle/commit/c9d931e949d1ebbc8efa68a8f93df49e97054d4e))
- **opencode** Reuse the shared search-before-answer and checkpoint skills - ([840f013](https://github.com/memcastle/memcastle/commit/840f0139e8448c0f0d2a378a59ceb9ebae0749b4))
- **opencode** Wake up on session start - ([76d4619](https://github.com/memcastle/memcastle/commit/76d4619a7453f70686201dc6abb45b0604ece28a))
- **opencode** Support OpenCode 1 and OpenCode 2 from one package ([#153](https://github.com/memcastle/memcastle/issues/153)) - ([9b1b864](https://github.com/memcastle/memcastle/commit/9b1b86497069b028e38154b5cf5599d0dedd5c76))
- **opencode** Keep each session's MCP connection alive and replace it when the daemon forgets it - ([5664a48](https://github.com/memcastle/memcastle/commit/5664a48266d9994f64c57ab627aa3a0537ffe476))
- **opencode** Scaffold the OpenCode integration plugin - ([02b89b1](https://github.com/memcastle/memcastle/commit/02b89b1bd43ba0deeb9856e061e2b06836d08777))
- **pi** Save an emergency checkpoint before context compaction ([#198](https://github.com/memcastle/memcastle/issues/198)) - ([f2f7ff7](https://github.com/memcastle/memcastle/commit/f2f7ff7955dbd501a7877c105fea0c2f46ca12fb))
- **pi** Checkpoint on an interval and on demand - ([e1c8c21](https://github.com/memcastle/memcastle/commit/e1c8c21c56cfbfeb71b81904065e5b3531db6472))
- **pi** Inject the search-before-answer skill every turn - ([5fe5349](https://github.com/memcastle/memcastle/commit/5fe53499bd9ff4cce64deb12eb42b4b98f19dd8b))
- **pi** Wake up on session start - ([bc7413f](https://github.com/memcastle/memcastle/commit/bc7413fc19efec2092f34278d59502c271cda7d6))
- **pi** Keep the MCP session alive and replace it when the daemon forgets it - ([b0fe088](https://github.com/memcastle/memcastle/commit/b0fe088a79c71f3876fb3d4208d26b470f108f44))
- **pi** Scaffold the Pi integration package - ([6f273c9](https://github.com/memcastle/memcastle/commit/6f273c95e586e712c2a3520058483f7205e6a85f))
- **search** Add temporal interval retrieval and drawer history ([#195](https://github.com/memcastle/memcastle/issues/195)) - ([c529763](https://github.com/memcastle/memcastle/commit/c52976359b74a0641193ea8da63676d26eb1ed35))
- **search** SurrealDB-native semantic, hybrid, temporal and graph-aware retrieval ([#149](https://github.com/memcastle/memcastle/issues/149)) - ([645539d](https://github.com/memcastle/memcastle/commit/645539df58e476012dc3fca8f474fcaec9ad2881))
- **skills** Distribute reusable agent skills with the repository ([#148](https://github.com/memcastle/memcastle/issues/148)) - ([0e1ca74](https://github.com/memcastle/memcastle/commit/0e1ca74a5dcb52b241f2b814400cfe0c3cace559))
- **sources** Distribute sources through registries and bundle the official ones ([#196](https://github.com/memcastle/memcastle/issues/196)) - ([0fb3c18](https://github.com/memcastle/memcastle/commit/0fb3c18d134d214ab1c925aaf0db8c467314d8d9))
- **sources** Add the OpenCode history source ([#194](https://github.com/memcastle/memcastle/issues/194)) - ([01369ad](https://github.com/memcastle/memcastle/commit/01369ad78d3a1309962457d7795ac4e472cadf50))

### 🐛 Bug Fixes

- **api** Accept `query` for the text of a POST /api/search body - ([2353d13](https://github.com/memcastle/memcastle/commit/2353d1391ea7b9343584c24a24b0b95aadcaa385))
- **app** Apply one limit rule to every read - ([d9db326](https://github.com/memcastle/memcastle/commit/d9db326654fa81bd46313263cdd1ea5aae31e3fc))
- **ci** List the package once before checking it, and gate the Unix-path test - ([868651c](https://github.com/memcastle/memcastle/commit/868651cef60a4b2893711355dc0514bc736a0d4a))
- **ci** Restore the pinned Rust toolchain in the release workflow ([#145](https://github.com/memcastle/memcastle/issues/145)) - ([0c6e4c5](https://github.com/memcastle/memcastle/commit/0c6e4c5b2ade2a91060ed1f601e42585bdb6a1cc))
- **cli** Stop `integration remove` accepting an --assets-dir it never reads - ([0b61f44](https://github.com/memcastle/memcastle/commit/0b61f441786234c3dd7ff3ebb9c1d0035d971f5a))
- **client** Report a malformed daemon address as a configuration error, not a retryable request failure - ([83f91ac](https://github.com/memcastle/memcastle/commit/83f91ac9288f85fb715becc8a96b2a765012b911))
- **integration** Build the installer's tests on Windows - ([1b2569f](https://github.com/memcastle/memcastle/commit/1b2569fa9ea80fa7f610fe04935e90ae55c4bac3))
- **integrations** Keep the invalid-input code list clear of the integrations guard - ([64dedc2](https://github.com/memcastle/memcastle/commit/64dedc2a93e10181c4e039490d212beae111ecaf))
- **integrations** Classify every invalid-input code as invalid input - ([a1f0ccd](https://github.com/memcastle/memcastle/commit/a1f0ccd6ad3a5e615050148f7990cdb3a53262d6))
- **mcp** Advertise every checkpoint item field - ([c3f9d70](https://github.com/memcastle/memcastle/commit/c3f9d70d1ec9301281e06a7947acbcc339981e20))
- **mining** Sanitise and validate the default wing like note does - ([1a7f93c](https://github.com/memcastle/memcastle/commit/1a7f93cc2ae8cd20d6b1c0b4090e7c97b337afb6))
- **source** Stop the bundle script deleting a component the wasm tests are reading ([#207](https://github.com/memcastle/memcastle/issues/207)) - ([5dc379f](https://github.com/memcastle/memcastle/commit/5dc379fe55ef733673e0fca115b3edd206ad53b5))
- **store** Retry drawer writes that lose a write conflict to index compaction ([#152](https://github.com/memcastle/memcastle/issues/152)) - ([98cb747](https://github.com/memcastle/memcastle/commit/98cb7475f535655671cdc0dd7278c3acaa2fb880))
- Name the right room, operation and URL segments in small API inconsistencies - ([97f0e4a](https://github.com/memcastle/memcastle/commit/97f0e4a9ffc98356d4e08fe4d92612c2cdeb54c4))

### 🔨 Refactor

- **domain** Derive thiserror for PackageTransitionError and word it with the names source list shows - ([7c0128c](https://github.com/memcastle/memcastle/commit/7c0128cfa203dede7f1d9cd07862ba8117762c55))
- **error** Share one drawer and entity id parser between REST and MCP - ([dfda49b](https://github.com/memcastle/memcastle/commit/dfda49b4dac4c5e56165ce07139b47b33827971c))
- **error**  🚨 **breaking** One word order and no implementation layers in diagnostic codes - ([f568cbd](https://github.com/memcastle/memcastle/commit/f568cbdb78c62a2d5d7c9637e24f37f64f8bead1))
- **migrate** Squash the unreleased data migrations into one version 3 step - ([347c67b](https://github.com/memcastle/memcastle/commit/347c67b4a0b5314ca30e772b0d82e7356143a83e))

### 📚 Documentation

- **adr** Carry every amendment in the index and cross-reference ADR-024, 031 and 033 - ([8a3f180](https://github.com/memcastle/memcastle/commit/8a3f1806e9acdb4134a742263cff0340a9bc8489))
- **agents** Add scope and cost rules for agents ([#180](https://github.com/memcastle/memcastle/issues/180)) - ([84973a3](https://github.com/memcastle/memcastle/commit/84973a3a3514dc34bd8422ced5eafb37e400fbb4))
- **api** Correct comments that overstated MCP coverage and the header casing, and complete the memory-mode matrix - ([e58d2be](https://github.com/memcastle/memcastle/commit/e58d2be8933a240babca9ef073a82100c1ec283b))
- **api** Add GET /api/sources and the full registry preview shape - ([f1d0997](https://github.com/memcastle/memcastle/commit/f1d0997732583b67c0935e9ae6559a228f6469d7))
- **api** List the embed job type in POST /api/jobs - ([a4ab65e](https://github.com/memcastle/memcastle/commit/a4ab65e4ee0f2cfd6afa20c7a44c681c1f9147fa))
- **cli** Correct which source commands are local, which print JSON, and what --mode does - ([3517039](https://github.com/memcastle/memcastle/commit/3517039ac21d97d88d20977e8807550287693056))
- **cli** Source install and update refuse without a terminal instead of proceeding - ([f14eb11](https://github.com/memcastle/memcastle/commit/f14eb110bd86cd43f366b9b97c2742e8372bb518))
- **daemon** Daemon start never replaces a degraded daemon - ([f2bb155](https://github.com/memcastle/memcastle/commit/f2bb15525eacc34691b26af1f212b750cafdf3e5))
- **integrations** Drop the unused Foundation status and stale scaffold wording, list project-core - ([b90f5e1](https://github.com/memcastle/memcastle/commit/b90f5e14eaaf2d3b0d5178ae8b800cb60f3e4adf))
- **integrations** MEMCASTLE_MODE takes different labels in the CLI and in Pi and OpenCode - ([d7529c5](https://github.com/memcastle/memcastle/commit/d7529c5092fe34c8125fc59b57cf5679d996c9cb))
- **integrations** Document the MCP session lifetime and recovery - ([8f8f854](https://github.com/memcastle/memcastle/commit/8f8f854784c7c304d0e14ddd19b4718e38aed608))
- **integrations** Define the shared Pi/OpenCode integration contract ([#147](https://github.com/memcastle/memcastle/issues/147)) - ([c937e0f](https://github.com/memcastle/memcastle/commit/c937e0fc3fa3be89b0315bdaf2ddd0281408c615))
- **mcp-clients** Warn against adding mcp.memcastle next to the OpenCode plugin - ([f101364](https://github.com/memcastle/memcastle/commit/f101364313fca8b2d96ed9a0910ff0bd013b7d0e))
- **migrations** Describe the renamed-fields piece and wrap long lines - ([4f2222a](https://github.com/memcastle/memcastle/commit/4f2222a7b0e833f82027512ac156219d1bc91558))
- **opencode** Map OpenCode extension mechanisms to MemCastle operations - ([b9e3026](https://github.com/memcastle/memcastle/commit/b9e3026f43bc488ee9a8507c1cbec5bb93d16d8d))
- **plan** Say the platform adapters are planned as WebAssembly sources, and #42 is done - ([572c17a](https://github.com/memcastle/memcastle/commit/572c17a3b67520e2ad3fd851f29992c28fbd2b40))
- **project-config** An explicit wake-up source outranks the project file - ([8cfedf4](https://github.com/memcastle/memcastle/commit/8cfedf4abb8be750c7987a60c765a28eccec3783))
- **skills** Name memcastle_history and qualify lexical matching - ([b8632b6](https://github.com/memcastle/memcastle/commit/b8632b6c6c68523061a7f9237cccac4d50247221))
- **test** Record the CI timings after merging the in-process binaries - ([c946d7b](https://github.com/memcastle/memcastle/commit/c946d7b3e5fbf665cc66ca2b56a6a60b0620e94f))
- **test** Record the CI timings after the shared build - ([1eb7228](https://github.com/memcastle/memcastle/commit/1eb7228c09a54f0be33f3c47bdc2b36e2a19a9db))
- **test** Use the warm-cache timings in the CI table - ([47e5f26](https://github.com/memcastle/memcastle/commit/47e5f263c9e40aed865256d9d7fd97730fb37fca))
- **test** Record the CI timings of the basic and wasm suites - ([2ea20fc](https://github.com/memcastle/memcastle/commit/2ea20fcc2656156ab7bbfd003e940f57155fba8d))
- Bring the README, skills README, ADR index, task table and layout up to date - ([464c559](https://github.com/memcastle/memcastle/commit/464c5590f99fc10ce1c652a4a9b67015d6366c9b))
- Correct the comments on audit gating and on what extraction reads - ([20600d2](https://github.com/memcastle/memcastle/commit/20600d24b163d5d52d85e2345c74a9686ac51de2))
- Name the integration commands among those that need no daemon - ([ba84d60](https://github.com/memcastle/memcastle/commit/ba84d60f38f0f1b98a55e3e8918f1069090f2a1d))
- Stop calling the bundled sources the only package asset - ([6a3dd1b](https://github.com/memcastle/memcastle/commit/6a3dd1bf50c2695e12aaae27ce7dfa3fa10f30a8))
- List integrations and skills in the release package layout - ([91925ea](https://github.com/memcastle/memcastle/commit/91925ea5cdc8d3729bcdf22224dbc8bb6ce23241))
- Show 3/3 migrations in the status samples - ([2646e17](https://github.com/memcastle/memcastle/commit/2646e17c3312fc8f46619a69f81660d479fc5397))
- Say what mise run ci runs, list the missing mise tasks and the pages the indexes skipped - ([e6a9370](https://github.com/memcastle/memcastle/commit/e6a9370a1745eecdf23d456d864b39f0040aa8a8))
- Correct stale code comments and module docs - ([7eaaa3f](https://github.com/memcastle/memcastle/commit/7eaaa3f2a0df8e1c3bdd41764827e3cefa2b24eb))
- Complete the contributor guides and add the missing ADR amendment notes - ([7ba4cc5](https://github.com/memcastle/memcastle/commit/7ba4cc566ad3a01dd07f5cbad3c58340a5574a87))
- Align the CLI, mode and extraction pages with the code - ([af70cc1](https://github.com/memcastle/memcastle/commit/af70cc16420d52e2536576a7a65fceaa8b0ff81a))
- List the supersession-lineage migration - ([5c8c57b](https://github.com/memcastle/memcastle/commit/5c8c57b1f29f156a2e0814437e48a3f7de335ba2))
- Describe the bundled sources in the install tree - ([25860e1](https://github.com/memcastle/memcastle/commit/25860e1f4f0df6f2bdc85e8d5b8d2923e6013fc0))

### 🧪 Tests

- **auth** Guard the drawer history route - ([f8bb332](https://github.com/memcastle/memcastle/commit/f8bb332b82b25ecdcc34c70aef012f3d39ebab28))
- **integration** Cover the installer's failure paths and move the command into the library - ([3d8963f](https://github.com/memcastle/memcastle/commit/3d8963f949693e372367814f2932fc5e62daf575))
- **integrations** Hold the failure and mode copies to be identical - ([13cb942](https://github.com/memcastle/memcastle/commit/13cb942ceded43e737b832588c1c2e638d2b948b))
- **wasm** Build components into one shared target and lockfile - ([ec15471](https://github.com/memcastle/memcastle/commit/ec154716d6ad02a3f1c4284013c99d784239131d))
- Forbid the source names that exist today in the pipeline and the domain model - ([22a4830](https://github.com/memcastle/memcastle/commit/22a4830b5f20a0b67c5120478f08b954723828d2))
- Run the in-process daemon tests as one binary - ([be09c14](https://github.com/memcastle/memcastle/commit/be09c147be532541dace824c301f9a10310b5437))

### 🏗️ Build

- **deps** Bump surrealkit ([#190](https://github.com/memcastle/memcastle/issues/190)) - ([9aa7d97](https://github.com/memcastle/memcastle/commit/9aa7d97f4ea57576f13b2e12e60985ad25cb0ecf))

### 🔧 CI

- **test** Build only the wasm test binaries in the wasm suite - ([a5d4cd5](https://github.com/memcastle/memcastle/commit/a5d4cd5c9453410ef6cbec742812525b2c304368))
- **test** Split the WebAssembly tests into their own suite and CI jobs ([#184](https://github.com/memcastle/memcastle/issues/184)) - ([384048b](https://github.com/memcastle/memcastle/commit/384048b77c43645271b5eeeeca1bdc756b4580dc))

## [0.2.0](https://github.com/memcastle/memcastle/compare/0.1.0..0.2.0) - 2026-10-03

### 💫 Features

- **cli** Manage wings, rooms and drawers ([#135](https://github.com/memcastle/memcastle/issues/135)) - ([7b20ffe](https://github.com/memcastle/memcastle/commit/7b20ffeb9a2fdf8e2d1933306b189779ca70fd1f))
- **cli**  🚨 **breaking** Group daemon start, stop and restart under `memcastle daemon` ([#132](https://github.com/memcastle/memcastle/issues/132)) - ([190b3d7](https://github.com/memcastle/memcastle/commit/190b3d7cd94e18a55973d09ec21534a364cc7db4))
- **cli** Colored help, shell completion, tables, prompts and styled status ([#131](https://github.com/memcastle/memcastle/issues/131)) - ([730a419](https://github.com/memcastle/memcastle/commit/730a4191eaa11f7ad5f24cb6bac52364e1daa5a3))
- **store** Add a store.sync setting for how often an embedded palace flushes - ([21da143](https://github.com/memcastle/memcastle/commit/21da1437d78b316bc7c535a9baa577d13ab7a39e))

### 🐛 Bug Fixes

- **api** Treat a database endpoint bind failure as a conflict and correct stale comments - ([ce5d5f8](https://github.com/memcastle/memcastle/commit/ce5d5f8fdbce2bebef0e40c099b8267812c216e4))
- **app** Refuse a fact confidence outside 0 to 1 at checkpoint submission - ([b081862](https://github.com/memcastle/memcastle/commit/b08186295f29ec5f360c9cec77e21e8ca3c413a6))
- **app** Refuse a blank diary entry like every other drawer writer - ([df1f643](https://github.com/memcastle/memcastle/commit/df1f643c0ad79181b5720974db2af936551409c7))
- **app** Validate new wing names on mine, checkpoint and diary writes - ([2547edb](https://github.com/memcastle/memcastle/commit/2547edb664191d07adf98ca9b1226903e3a0d266))
- **config** Refuse an auth token with edge whitespace in the config file - ([c7ec275](https://github.com/memcastle/memcastle/commit/c7ec2752d76ce6df48419f9728e17020919f06c5))
- **dbadmin** Accept the Studio desktop app's origin and sign in as a fixed user ([#129](https://github.com/memcastle/memcastle/issues/129)) - ([5ccfb5e](https://github.com/memcastle/memcastle/commit/5ccfb5e3f93628134d19cc669b72dbd924904ab9))
- **error** Give job contention and missing entropy their own diagnostics - ([682bf34](https://github.com/memcastle/memcastle/commit/682bf34516c0be375c5fa7ec7810e8fecee274d3))
- **hooks** Make store-isolation fail when either of its checks matches - ([c476ba2](https://github.com/memcastle/memcastle/commit/c476ba2682d2b69f3a1eac7275cb3d85d444ea95))
- **mcp** Advertise an object schema for the checkpoint payload and accept it as a JSON string ([#134](https://github.com/memcastle/memcastle/issues/134)) - ([6023297](https://github.com/memcastle/memcastle/commit/60232976175d52923317e18dd3c69b533a1120e7))
- **packaging** Expand the binary path in the nfpm config ([#121](https://github.com/memcastle/memcastle/issues/121)) - ([d0d8912](https://github.com/memcastle/memcastle/commit/d0d89120fb17ef6a51f90ddd7f468409274529f6))
- **search** Fall back to any-word matching and reject empty checkpoints ([#137](https://github.com/memcastle/memcastle/issues/137)) - ([ec7f425](https://github.com/memcastle/memcastle/commit/ec7f4251e7d19513ec0f48585d81d1f57f267d7a))
- **server** Wait for the embedded database to stop before exiting ([#133](https://github.com/memcastle/memcastle/issues/133)) - ([2ebc989](https://github.com/memcastle/memcastle/commit/2ebc989ff3033f1dcecd9d74aa73d21472305634))

### ⚡ Performance

- **migrate** Skip the second schema sync when no data migration ran - ([b1a434a](https://github.com/memcastle/memcastle/commit/b1a434ad56f8bf4cbe94264c844a95f89f602f40))

### 🔨 Refactor

- **cli** Rename the jobs subcommand to job ([#136](https://github.com/memcastle/memcastle/issues/136)) - ([20897e3](https://github.com/memcastle/memcastle/commit/20897e3175e6372b352d04a620a49d0641a75b03))
- **db**  🚨 **breaking** Rename db serve to db start and make it idempotent ([#130](https://github.com/memcastle/memcastle/issues/130)) - ([6262c40](https://github.com/memcastle/memcastle/commit/6262c40991aad988c02878cd7aa67bc746116847))

### 📚 Documentation

- **architecture** Say the MCP surface mirrors memory operations only - ([6305c91](https://github.com/memcastle/memcastle/commit/6305c913db07706b999b81a450f4bbee6fcc4dc8))
- **development** Use daemon stop in the run-locally example - ([86c17b8](https://github.com/memcastle/memcastle/commit/86c17b83a6a71da080e7445b1fbfbef75aed269e))
- **installation** List the completion scripts the packages install - ([79948a0](https://github.com/memcastle/memcastle/commit/79948a0ab7518f6494932ae09b20608aded7a2d2))
- **mcp-and-api** Say what a checkpoint source kind other than file or manual becomes - ([6f9ee61](https://github.com/memcastle/memcastle/commit/6f9ee61820c00bbfeee56e7a2b35fee97c3aab16))
- **troubleshooting** Document the remaining diagnostics and start the daemon with daemon start - ([d63d2ac](https://github.com/memcastle/memcastle/commit/d63d2acfe0cf9be7c11995f1dba86cf3704fefd6))
- Keep the reflowed lines within the 120-column limit - ([1a953c1](https://github.com/memcastle/memcastle/commit/1a953c1c8c0dd52e48f8c5bc3cb8b3427b53410a))
- One sentence per line in AGENTS.md, CONTRIBUTING.md and ADR-001, 002 and 004 - ([342d433](https://github.com/memcastle/memcastle/commit/342d433c1a96c4092b98dfeb1e2758806aca3867))
- Align layout, defaults, ADR pointers, test list and guide order with the code - ([b73e161](https://github.com/memcastle/memcastle/commit/b73e1614db215bbe63ddd6f12ce501f01d3a3e52))

### 🧪 Tests

- **palace** Cancel the queued mine before the blocker so it cannot slip into running - ([5d007d0](https://github.com/memcastle/memcastle/commit/5d007d062fa9d539bd160bef9b64f3fb6e924101))
- Skip the per-commit flush where a test is not about durability - ([85e4dbe](https://github.com/memcastle/memcastle/commit/85e4dbe3e35ac2500e748271588e806747a7f9b1))

### 🏗️ Build

- **deps** Bump comfy-table from 7.2.2 to 8.0.1 ([#142](https://github.com/memcastle/memcastle/issues/142)) - ([3a7973d](https://github.com/memcastle/memcastle/commit/3a7973d28c8e24b6d0d45cd6a7e5f2e42933075a))
- **deps** Bump rmcp ([#139](https://github.com/memcastle/memcastle/issues/139)) - ([fb6a3f3](https://github.com/memcastle/memcastle/commit/fb6a3f3591949f86172a7abd7743e54224a30eb8))
- **deps** Bump tokio-tungstenite from 0.29.0 to 0.30.0 ([#141](https://github.com/memcastle/memcastle/issues/141)) - ([6fb8b4f](https://github.com/memcastle/memcastle/commit/6fb8b4f6914c376bde58f2161a2a48de00420bde))
- **deps** Bump tower-http from 0.6.11 to 0.7.1 ([#140](https://github.com/memcastle/memcastle/issues/140)) - ([06af749](https://github.com/memcastle/memcastle/commit/06af7491b9dc5f66fa40e6930e0909fe7deaab7f))

### 🔧 CI

- **deps** Bump the actions group with 2 updates ([#138](https://github.com/memcastle/memcastle/issues/138)) - ([2b04579](https://github.com/memcastle/memcastle/commit/2b04579368ca2fdcdb484959bd4e39516160b551))
- Keep Windows test temp files on the workspace drive - ([549c9c0](https://github.com/memcastle/memcastle/commit/549c9c0df5556dc8d25628ccb5199b96653e6838))

## 0.1.0 - 2026-10-02

### 💫 Features

- **app** Add MemoryMode with per-session enforcement ([#55](https://github.com/memcastle/memcastle/issues/55)) - ([b2af56c](https://github.com/memcastle/memcastle/commit/b2af56c86fd4e54bd0089548c66ecf32e29a16a6))
- **app** Add AppServices::recall and AppServices::wake_up ([#54](https://github.com/memcastle/memcastle/issues/54)) - ([daab62e](https://github.com/memcastle/memcastle/commit/daab62ef5b67ec8a3ae65f8037b8214cfbe7a49d))
- **app** Add AppServices::diary_write / diary_read (direct call) ([#53](https://github.com/memcastle/memcastle/issues/53)) - ([e6eda2f](https://github.com/memcastle/memcastle/commit/e6eda2f89cee440a8049aef5eb66d88aad02a4d5))
- **audit** Add JobKind::Audit, a read-only palace consistency report ([#56](https://github.com/memcastle/memcastle/issues/56)) - ([8d1b7e1](https://github.com/memcastle/memcastle/commit/8d1b7e1e9fa9407ccad6409de99b4b73b11b38f2))
- **auth** Add optional token authentication for the daemon ([#111](https://github.com/memcastle/memcastle/issues/111)) - ([ce30604](https://github.com/memcastle/memcastle/commit/ce30604a99de5a26cdbb33e5543be7214332e2dd))
- **checkpoint** Add JobKind::Checkpoint and its handler ([#52](https://github.com/memcastle/memcastle/issues/52)) - ([6c2cea6](https://github.com/memcastle/memcastle/commit/6c2cea6190700f61a443ed78a12f70a24fd9a510))
- **cli**  🚨 **breaking** Report daemon, datastore and endpoint state from memcastle status ([#104](https://github.com/memcastle/memcastle/issues/104)) - ([b27b9a0](https://github.com/memcastle/memcastle/commit/b27b9a0125f18509c2c0f156765d333e83d3b400))
- **config**  🚨 **breaking** Configurable daemon bind address and port ([#103](https://github.com/memcastle/memcastle/issues/103)) - ([ddd874e](https://github.com/memcastle/memcastle/commit/ddd874e7b641b731ebb667316b0901c0409ab80a))
- **config** Resolve config, data and state paths from Unix XDG dirs ([#102](https://github.com/memcastle/memcastle/issues/102)) - ([92a5da5](https://github.com/memcastle/memcastle/commit/92a5da533d63a4f7940e21fdeb47e8d76cb81cd2))
- **db** Expose the embedded SurrealDB through an opt-in admin endpoint ([#118](https://github.com/memcastle/memcastle/issues/118)) - ([18dc808](https://github.com/memcastle/memcastle/commit/18dc8080f55ffb267edb9bcd7d4fe035bb1fa881))
- **domain** Add Priority enum and rework job priority representation ([#46](https://github.com/memcastle/memcastle/issues/46)) - ([c52cc89](https://github.com/memcastle/memcastle/commit/c52cc892f5963ff8a50af63265557999ca1a8bbb))
- **jobs** Hold running jobs by a heartbeat lease so a remote palace can be shared (ADR-006) - ([6c92cad](https://github.com/memcastle/memcastle/commit/6c92cad430db601aa07e5b0afe246e98ead664d2))
- **jobs** Let audit and repair pause and stop on shutdown; configurable drain timeout - ([0068900](https://github.com/memcastle/memcastle/commit/00689009f34b4b17d4ecd8b1152ed585a1dff4f0))
- **logging** Add structured daemon logging with JSON output and job lifecycle events - ([27890cd](https://github.com/memcastle/memcastle/commit/27890cdf8278c8f0e0cb1a0ab36752e51d2840e0))
- **mcp** Expose repair and job control, and generate the server instructions - ([d9754aa](https://github.com/memcastle/memcastle/commit/d9754aa04ae65095fe13aa19d6c13da8fd81ad99))
- **migrate** Add versioned data migrations and SurrealKit-backed schema sync ([#62](https://github.com/memcastle/memcastle/issues/62)) - ([32c6ba6](https://github.com/memcastle/memcastle/commit/32c6ba6d910987b706ff892b6d5ad2b461e34377))
- **packaging** Ship the systemd user unit in the .deb and .rpm - ([0545cf8](https://github.com/memcastle/memcastle/commit/0545cf8c5c33c0e63cf1e91abd050b36fcf27881))
- **packaging** Build .deb and .rpm release packages with nfpm - ([cab0be6](https://github.com/memcastle/memcastle/commit/cab0be6986c78c54556eff1201f77be69839ce24))
- **packaging** Add Arch systemd user unit and authentication docs - ([b6fae2e](https://github.com/memcastle/memcastle/commit/b6fae2e112c593870125a55230bc8ee80627d07f))
- **release** Add runtime asset resolution and reproducible release packaging ([#109](https://github.com/memcastle/memcastle/issues/109)) - ([e6220b2](https://github.com/memcastle/memcastle/commit/e6220b2e86dca946971b9f81e5ab3a97ad58f675))
- **release** Enable AUR and Homebrew publishing - ([b941872](https://github.com/memcastle/memcastle/commit/b941872ded4a93c5978d6bf61dc6c674c8fb1af5))
- **repair** Add JobKind::Repair, a narrow dry-run-first repair job ([#57](https://github.com/memcastle/memcastle/issues/57)) - ([fad38e4](https://github.com/memcastle/memcastle/commit/fad38e4280499957147a1c549c42031c433f1d92))
- **search** Add wing/room scope to lexical_search ([#50](https://github.com/memcastle/memcastle/issues/50)) - ([d648211](https://github.com/memcastle/memcastle/commit/d648211aeda35db3ac834340800e7b9256a79018))
- **store** Add knowledge-graph entity/relationship operations ([#51](https://github.com/memcastle/memcastle/issues/51)) - ([e762721](https://github.com/memcastle/memcastle/commit/e762721da08a71e221b5edc3d97bda273ed3a5a5))
- **store** Use surrealkv as the only embedded storage backend ([#49](https://github.com/memcastle/memcastle/issues/49)) - ([57de598](https://github.com/memcastle/memcastle/commit/57de59838dda9301e284ded84ff4a94a214ddac3))
- One error and logging surface for MCP, REST and the CLI - ([442507c](https://github.com/memcastle/memcastle/commit/442507c983fbc0b256a50ca2ef424d6c29f7a54e))
- Implement the core daemon architecture - ([1a2bb0a](https://github.com/memcastle/memcastle/commit/1a2bb0ae6d3794996b2257017cea94db28a36616))

### 🐛 Bug Fixes

- **api** Report bad job ids and checkpoint payloads like MCP and the CLI - ([0a025cf](https://github.com/memcastle/memcastle/commit/0a025cf20327a3db4b1a14158921c82a4a7c2618))
- **api** Report malformed requests with the shared error body - ([a3ed48a](https://github.com/memcastle/memcastle/commit/a3ed48a881391bc01dce5395ff2f4f79694cb9e3))
- **api** Structured error bodies and honest client errors - ([f03d8ae](https://github.com/memcastle/memcastle/commit/f03d8ae51c1eaeca3a63106f372fb2a6b1d4075c))
- **api** Default repair dry_run to true when omitted - ([102d787](https://github.com/memcastle/memcastle/commit/102d787e9f44a8c7bd7af3b8c23053849cd88e01))
- **app** Reject relative mine paths at submission and name recall in its own rejection - ([976da90](https://github.com/memcastle/memcastle/commit/976da908868a57f1e2043f12afa0bae5c7ea8126))
- **app** Give provenance.requested_by and source.agent one meaning across writers - ([9a97e22](https://github.com/memcastle/memcastle/commit/9a97e226c38da80e55419e32ae0ecac1d8b4ed0e))
- **app** Read-only operations must not write - ([e2c2033](https://github.com/memcastle/memcastle/commit/e2c20331611c2a859ae928aa70fb8d71a8f2549e))
- **app** Stop leaking checkpoint content through job endpoints - ([208c542](https://github.com/memcastle/memcastle/commit/208c542861913a73d1a6d6eec16b47a955c441fc))
- **ci** Regenerate mise.lock with full platform lockfile data - ([53a5e8a](https://github.com/memcastle/memcastle/commit/53a5e8a777c8c5d920f7f4773ff0e18db72cf28c))
- **cli** Wait for the old daemon to be gone before restarting; portable mining test - ([ee35973](https://github.com/memcastle/memcastle/commit/ee3597396ae24e710c32330a86e31e17f11c4871))
- **cli** Restart flags, absolute mine paths, working -v, and --mode - ([b4db200](https://github.com/memcastle/memcastle/commit/b4db2008c1444ce268ad9d5e327ae79bb37b4b38))
- **config** Redact the remote database password like the auth token - ([d785b93](https://github.com/memcastle/memcastle/commit/d785b9384598d4aa9114ce02a5f3b9a70a562d59))
- **config** Reject malformed MEMCASTLE_* overrides instead of silently ignoring them - ([e2f31f3](https://github.com/memcastle/memcastle/commit/e2f31f340173b7599b0270cba6c3bd0cf456b169))
- **config** Honour logging.level when initializing tracing - ([2e0e7d2](https://github.com/memcastle/memcastle/commit/2e0e7d28299a389e3e2aa9852bcd3dad4b787495))
- **error** Name statuses, events and modes by their wire names in messages - ([60ba139](https://github.com/memcastle/memcastle/commit/60ba139f6d341bdf8d3a988d30d4027ed1064386))
- **hooks** Catch multi-line grouped imports and all DDL kinds - ([e369acd](https://github.com/memcastle/memcastle/commit/e369acd06860f2ddefefd93731c00c9cf78febed))
- **jobs** Guard cancel, resume and retry on the status they read - ([2dc82db](https://github.com/memcastle/memcastle/commit/2dc82db114ebdb079ca7c11cb9e0f54e9dd01566))
- **jobs** Count crash recoveries, not every claim, against the attempt budget - ([3ae9cf5](https://github.com/memcastle/memcastle/commit/3ae9cf52ace524c32af69c2c9118b964e755d61e))
- **jobs** Persist pause and cancel requests - ([253b5ba](https://github.com/memcastle/memcastle/commit/253b5ba1b88d71b55153a04758139b526dffc598))
- **jobs** Make resume after a crash idempotent - ([8bb8c08](https://github.com/memcastle/memcastle/commit/8bb8c086c217e73170b6cd7fb6ac8dca8efa274e))
- **jobs** Release the lease when a job leaves Running - ([82f1fd1](https://github.com/memcastle/memcastle/commit/82f1fd14ebbd170db15f95a81ffd52fb21a111d4))
- **jobs** Retry a failed job through Job::apply instead of a hand-rolled mutation ([#48](https://github.com/memcastle/memcastle/issues/48)) - ([af45b11](https://github.com/memcastle/memcastle/commit/af45b11797506b3d10fab568a6733ea34c546955))
- **mcp** Scope a session's mode to that session and refuse headerless set_mode - ([9762e94](https://github.com/memcastle/memcastle/commit/9762e9437ae59fd187bc76b071f1ebe0bd07ef26))
- **repair** Validate based_on_job when a repair is submitted - ([0d12602](https://github.com/memcastle/memcastle/commit/0d12602cbb62109b8c8681f6664509aee5a68b85))
- **server** Drain in-flight jobs on shutdown - ([bc000bd](https://github.com/memcastle/memcastle/commit/bc000bdd1aefa8e5091f69ca2946d106f9dac98d))
- **store** Retry job writes that lose a SurrealDB write conflict - ([5344d7b](https://github.com/memcastle/memcastle/commit/5344d7b44df9c8e3fc82f33ba62ed63213ef9ae6))
- **test** Poll for job completion instead of a fixed sleep - ([1b8a55b](https://github.com/memcastle/memcastle/commit/1b8a55bf31620566f88553ec3ad799e3503bfc75))
- Align the REST, MCP and client surfaces on shared types and behaviour - ([5b2f694](https://github.com/memcastle/memcastle/commit/5b2f69457847e76d195f078a6184e8339687866c))
- Report truncation, skip oversize wake-up highlights, reject missing relationships - ([07983ac](https://github.com/memcastle/memcastle/commit/07983aca4bb09dd88dc07cc8f661099f5b288e32))

### 🔨 Refactor

- **app** Use the submit_ prefix for every job-submitting method and share the repair default - ([201a620](https://github.com/memcastle/memcastle/commit/201a62011a070721b92517644c293f48196b9575))
- **app**  🚨 **breaking** Consistent AppServices and DaemonClient signatures - ([02cdeb6](https://github.com/memcastle/memcastle/commit/02cdeb641a2f3e446d204c994eedead2b119751f))
- **error**  🚨 **breaking** Diagnostic code and variant hygiene - ([e6b5590](https://github.com/memcastle/memcastle/commit/e6b5590c7affe2beb6f2bd69ff3cc6a16a954424))
- **error** Typed diagnostics instead of Error::Config misuse - ([4c39da9](https://github.com/memcastle/memcastle/commit/4c39da9f407eff21fde2b47ea76cb9617c4d2a20))
- **jobs** Share the stop check and the resume-index parse across handlers - ([1b5464b](https://github.com/memcastle/memcastle/commit/1b5464b6ccb1a246a61f6eb5cfc5420454624bc2))
- **jobs** Give every job handler the run(ctx, job, params) shape - ([f3f65c2](https://github.com/memcastle/memcastle/commit/f3f65c24db42c43d41025401fb4ce7cb29666ba4))
- **mcp** Name the job list tool and mode labels consistently with job_get - ([ea3f089](https://github.com/memcastle/memcastle/commit/ea3f08939630a93dfb6ff4b39e4118916f912046))
- **mining** Introduce a MiningSource seam behind JobKind::Mine ([#58](https://github.com/memcastle/memcastle/issues/58)) - ([8b9d531](https://github.com/memcastle/memcastle/commit/8b9d5311fd84d4b9ed9eaf1b37b70cb1aa4d7dcf))
- **store** Pin one canonical timestamp form and record why (ADR-005) - ([1638ece](https://github.com/memcastle/memcastle/commit/1638ece9be248bde9e13f5317e54ec6c0cd318de))
- **store** Consistent naming, ownership and no duplicate methods - ([478763d](https://github.com/memcastle/memcastle/commit/478763d742ad5d06b796b8db1b84d788e54d2903))
- Share scheduler defaults and the diary room name, and declare tokio's net feature - ([610171e](https://github.com/memcastle/memcastle/commit/610171ebe0ed482ca54f01b232df9321f61a374f))
- Share the job id and status parsers and the channel names across cli, api and mcp - ([15ed33f](https://github.com/memcastle/memcastle/commit/15ed33f93eb5e7d79bd98624a642a13568f18ac2))
- Share drawer construction, checkpointing and defaults - ([e2c6926](https://github.com/memcastle/memcastle/commit/e2c6926a3ddd7b9ce08df48a0b4da77b8724edad))

### 📚 Documentation

- **adr** Date the amendments and point to them from Status and the index - ([74d407b](https://github.com/memcastle/memcastle/commit/74d407b8f1cb72b26a2ded662c59eca5f98d272e))
- **adr** Mark ADR-011's rejected warning as reversed - ([d39a136](https://github.com/memcastle/memcastle/commit/d39a1364db7ea9855bf6f55cda7a031fcf634754))
- **adr** Align ADR-010 with the documented Windows support - ([ae73c16](https://github.com/memcastle/memcastle/commit/ae73c16786202ca6f42dec6320ddbc39874282be))
- **adr** Mark ADR-002's gated-operation list as amended by ADR-007 - ([d258095](https://github.com/memcastle/memcastle/commit/d258095c5cb6191d068650cec75d3ec9f22096de))
- **adr** Record the Phase 1 hardening decisions - ([6267b82](https://github.com/memcastle/memcastle/commit/6267b826b7610bef277f2425d8d503332214197d))
- **agents** List auth-token among the REST routes - ([b1f06b8](https://github.com/memcastle/memcastle/commit/b1f06b82a568ddc25b261915ec196082745d36ff))
- **api** Name job reads among the gated routes - ([292eef6](https://github.com/memcastle/memcastle/commit/292eef69c4ddd9d48e6b96ffd1c4eb823416f927))
- **architecture** Stop listing .deb/.rpm packages as a non-goal - ([b241478](https://github.com/memcastle/memcastle/commit/b24147852000106c3f2ab7ac0e15eab4104e2d58))
- **architecture** Document Phase 1 decisions and add ADRs ([#59](https://github.com/memcastle/memcastle/issues/59)) - ([947c357](https://github.com/memcastle/memcastle/commit/947c357984efd6579bc43616ba256de20f5500e2))
- **auth** State that auth::not_configured fires after migrations - ([ca61891](https://github.com/memcastle/memcastle/commit/ca61891e016ebccf632028038749200c8f02b802))
- **cli** List the commands that do not fail with not_running or print only JSON - ([5cfb3a8](https://github.com/memcastle/memcastle/commit/5cfb3a8c095ace95b3d3fa4fb47e029532890b99))
- **code** Correct stale doc comments that contradicted the code or the docs - ([a28faca](https://github.com/memcastle/memcastle/commit/a28faca75008bbfe12dc84a92edbd9ef4104f380))
- **config** Correct the section count in the module comment - ([c2c312e](https://github.com/memcastle/memcastle/commit/c2c312e22b018ce379cf31ec2ac9f588d8abb19a))
- **configuration** Align the --mode scope with the CLI reference - ([60832d3](https://github.com/memcastle/memcastle/commit/60832d3b5bac818bd46b9dd585738d5a5e92069c))
- **contributing** Use `mise cli` in the task table - ([61f7e80](https://github.com/memcastle/memcastle/commit/61f7e8084f50d37ead3a3f17eb6f9aaa87187672))
- **daemon** Job started logs the attempt, not the elapsed time - ([42bbd96](https://github.com/memcastle/memcastle/commit/42bbd96c8ee4d44b5ab8e754b9ed026413300c40))
- **daemon** Say what actually runs before the listener binds and when migrations run - ([4eb27a5](https://github.com/memcastle/memcastle/commit/4eb27a568130fffd3a58cef5f4ad7b3cbfee0ee4))
- **daemon** Note the packaged unit pins MEMCASTLE_LOG=warn - ([592e616](https://github.com/memcastle/memcastle/commit/592e616798bc45b8020f32f927aa90d74f0b1a07))
- **development** List config_assets and dependencies tests - ([750f060](https://github.com/memcastle/memcastle/commit/750f060bc7b6512c35b1e6e1304e82babaee239a))
- **development** Describe tests/auth.rs as walking a route list - ([82eb9e8](https://github.com/memcastle/memcastle/commit/82eb9e809342ae7f4549a55d0de62c126169e28a))
- **installation** Count the AUR package's licence file - ([1aa435f](https://github.com/memcastle/memcastle/commit/1aa435f4c81e5ddd4111af89517c0005a591e6b5))
- **logo** Added the Shaipe SVG and both icon renders ([#117](https://github.com/memcastle/memcastle/issues/117)) - ([ac9e360](https://github.com/memcastle/memcastle/commit/ac9e360a5bfebb33e6b6ce20cb15cb95b40f3da3))
- **scaffold** Call out non-goals in integrations/skills READMEs ([#60](https://github.com/memcastle/memcastle/issues/60)) - ([adef6d1](https://github.com/memcastle/memcastle/commit/adef6d1693d32a844e8f3165227fbacfb2948049))
- **storage** Say a second serve on the same port fails with bind_failed first - ([4b0ed0a](https://github.com/memcastle/memcastle/commit/4b0ed0a54ee4ae9b8e40084f7f166d02ac9271fa))
- **storage** Document the dot-directory and symlink skips in mining - ([c93210b](https://github.com/memcastle/memcastle/commit/c93210be8d361155d9ba900f1981ce89d3e16741))
- **storage** Remote stores use their configured namespace and database - ([1ba1ef0](https://github.com/memcastle/memcastle/commit/1ba1ef0192cdea8343a139271cf7795359d0d7e4))
- Link database access from the indexes and align the unit, hook and layering wording - ([1a13ec0](https://github.com/memcastle/memcastle/commit/1a13ec0cf52925918242084cb113df38195e62da))
- Rewrap lines to satisfy markdownlint - ([e24a4c6](https://github.com/memcastle/memcastle/commit/e24a4c6ac1c5917b46d39641bf1d90507dae6fa1))
- Say mise run ci also covers the docs build - ([3fe2160](https://github.com/memcastle/memcastle/commit/3fe2160a181b4ea21ea883c126a0c38d3074330c))
- Link the Authentication guide from the index and README - ([e8f6e01](https://github.com/memcastle/memcastle/commit/e8f6e01770a1d7e996afa1fa3bd7e5149f770046))
- Say mine needs an absolute path over REST as well as MCP (fixup) - ([c345b7f](https://github.com/memcastle/memcastle/commit/c345b7f85a33ef56876acae6ba61ca3ece52ae5e))
- Mention the /mcp Host-header restriction where binding is described - ([e752511](https://github.com/memcastle/memcastle/commit/e75251102137e130dd7561e4b4832991ec0618bc))
- Say mine needs an absolute path over REST as well as MCP - ([7e3ab15](https://github.com/memcastle/memcastle/commit/7e3ab150d5f085060369b704a43547cc8ad9be95))
- Show the auth line in the sample status output - ([967665f](https://github.com/memcastle/memcastle/commit/967665fcc15e86d4a71a387ff554840c812ba0de))
- List --assets-dir among the flags restart forwards - ([dc8405c](https://github.com/memcastle/memcastle/commit/dc8405c3e03d9fcac752dcbff1c60a191f2325a0))
- Overhaul user documentation and add architecture diagrams - ([3d5aa58](https://github.com/memcastle/memcastle/commit/3d5aa589b3b388c3800840f27639c877a9efd297))
- Correct stale docstrings and comments that no longer match the code - ([7219607](https://github.com/memcastle/memcastle/commit/7219607c49da7765a505bed2d692ae49fcf1540d))
- Reconcile the palace, ADR, recovery, server-deployment and configuration wording - ([2aafd7d](https://github.com/memcastle/memcastle/commit/2aafd7db9e571fb90ea2e9a4773ab51769c6c718))
- Name restart and migrate as the exceptions to the thin-client rule - ([bafd5e8](https://github.com/memcastle/memcastle/commit/bafd5e8a29f6071d4fdc1acf569167cd4e6596d0))
- Fix the toolchain and git-tpl pointers in the development guide - ([9a684f2](https://github.com/memcastle/memcastle/commit/9a684f2acbf88854c5c319feea39c55bc87976f4))
- Scope the one-writer invariant to embedded palaces - ([c3ae697](https://github.com/memcastle/memcastle/commit/c3ae697a1a2b119e664e7b8e6e4e2e00a397e3aa))
- Fix drift found by the consistency check - ([85460b1](https://github.com/memcastle/memcastle/commit/85460b1f1983432ce1b2ff7dbae9420138e79eaf))
- Document the migrate exception, retry transition and Phase 1 status - ([390dc44](https://github.com/memcastle/memcastle/commit/390dc448cb52b012735459013defad6315e2e4f3))
- Add V1 architecture roadmap and integrations/skills scaffold - ([91df823](https://github.com/memcastle/memcastle/commit/91df8235573967ec15ed6456ee45360c3eadbf1d))

### 🧪 Tests

- **concurrency** Compare completion times instead of racing a poll - ([434345b](https://github.com/memcastle/memcastle/commit/434345b12e6c9ef8dc3be59a906b6f951b0343ff))
- **jobs** Exercise Scheduler::recover for every job state - ([132e2cc](https://github.com/memcastle/memcastle/commit/132e2ccb74b00085f171856b6a2df1257860da31))
- **mcp** Add an end-to-end MCP smoke test ([#105](https://github.com/memcastle/memcastle/issues/105)) - ([95bf22f](https://github.com/memcastle/memcastle/commit/95bf22fe2a23ced41aad489f468ffeb6c8e7800d))
- **mcp** Cover recall, wake_up, diary and emergency checkpoint by mode - ([b5888ed](https://github.com/memcastle/memcastle/commit/b5888ed112f831e1773d8f778aa088aefb601604))
- Enforce invariants 2 and 5 with a full transition table and two guard hooks - ([0465a3b](https://github.com/memcastle/memcastle/commit/0465a3b76e6e1c2c4b6f90e3984e8f54065d005b))
- Capture restart's output through files so Windows does not hang - ([e4bd2a5](https://github.com/memcastle/memcastle/commit/e4bd2a5b3677c55ae394812031d9150f7e012639))
- Read diagnostic codes from the errors instead of spelling jobs codes in mcp and api - ([30709c6](https://github.com/memcastle/memcastle/commit/30709c61dc5d297ed743252987a740af0ff03196))

### 🏗️ Build

- Run the architecture guards in check and ci, and align how the docs describe them - ([37034f6](https://github.com/memcastle/memcastle/commit/37034f6eb2869eb0a94897eafe27c1dcbaa0a04e))

### 🔧 CI

- **guard** Widen the architecture guard to jobs, client and main.rs - ([31e0e88](https://github.com/memcastle/memcastle/commit/31e0e88d8c67322520d0bf24776f6ef155d6b940))
- Cache Rust compilation with sccache, faster linkers per OS ([#45](https://github.com/memcastle/memcastle/issues/45)) - ([8cf2120](https://github.com/memcastle/memcastle/commit/8cf2120dc274917482c9ab4dff8bb6c2c5c28448))

### 🧹 Chores

- Drop unused dependencies, assert what two CLI tests promise, share one job-wait helper - ([3da89bf](https://github.com/memcastle/memcastle/commit/3da89bf3251eb351a8b038c2d1bab6bc08e13fe7))
- Initialize project from rust.tpl - ([1fc71a6](https://github.com/memcastle/memcastle/commit/1fc71a6f98fb3a46c175dcc25a1083d0b9525495))

### Tpl

- Render rust at main - ([9e0948b](https://github.com/memcastle/memcastle/commit/9e0948bc1e48d5f5d0cc890df22428dd94996d39))
- Render rust at <worktree> - ([807de5e](https://github.com/memcastle/memcastle/commit/807de5e7307dc17a497a4c90bd09b5acc34b3c01))

## ❤️ New Contributors

* @noirbizarre made their first contribution in [#119](https://github.com/memcastle/memcastle/pull/119)
