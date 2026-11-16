# Import graph

Direct statically resolved imports within bot, including imported package initializers. Line numbers identify the import statement. Dynamic imports, wildcard exports and rebinding are not resolved. Decorators are shown as source expressions in source order (top to bottom); Python applies them bottom to top.

## `bot/__init__.py`

Imports: none

## `bot/__main__.py`

Imports: `bot/__init__.py:4`, `bot/runtime.py:4`

## `bot/compose/__init__.py`

Imports: `bot/__init__.py:7`, `bot/compose/__init__.py:507`, `bot/compose/badnews.py:507`, `bot/memory/__init__.py:7`, `bot/memory/history.py:7`

## `bot/compose/a_count_of_packets.py`

Imports: none

## `bot/compose/a_detail_dropped.py`

Imports: `bot/__init__.py:33`, `bot/compose/__init__.py:33`

## `bot/compose/a_document_lives_with_an_office.py`

Imports: `bot/__init__.py:26`, `bot/compose/__init__.py:26`

## `bot/compose/a_first_time.py`

Imports: `bot/__init__.py:15`, `bot/compose/__init__.py:15`, `bot/compose/fallback.py:15`

## `bot/compose/a_number_the_bot_knows.py`

Imports: none

## `bot/compose/a_place_nobody_can_see.py`

Imports: `bot/__init__.py:26`, `bot/compose/__init__.py:26`

## `bot/compose/a_thing_being_weighed.py`

Imports: `bot/__init__.py:40`, `bot/compose/__init__.py:40`

## `bot/compose/a_thing_wants_doing.py`

Imports: `bot/__init__.py:17`, `bot/compose/__init__.py:17`, `bot/compose/fallback_data.py:17`

## `bot/compose/a_trouble.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/compose/aboutness.py:23`, `bot/compose/fallback.py:155`

## `bot/compose/a_wait_about_a_body.py`

Imports: `bot/__init__.py:27`, `bot/compose/__init__.py:27`

## `bot/compose/aboutness.py`

Imports: none

## `bot/compose/badnews.py`

Imports: `bot/__init__.py:31`, `bot/compose/__init__.py:1335`, `bot/compose/goodnews.py:1335`, `bot/memory/__init__.py:27`, `bot/memory/dream.py:27`, `bot/memory/whose_body.py:1154`, `bot/understanding/__init__.py:31`, `bot/understanding/normalize.py:31`

## `bot/compose/breakup.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/compose/fallback.py:23`

## `bot/compose/check_with_you.py`

Imports: none

## `bot/compose/checking.py`

Imports: `bot/__init__.py:49`, `bot/compose/__init__.py:53`, `bot/knowledge/__init__.py:80`, `bot/knowledge/geography.py:80`, `bot/memory/__init__.py:49`, `bot/memory/facts.py:49`

## `bot/compose/childwords.py`

Imports: `bot/__init__.py:24`, `bot/compose/__init__.py:24`, `bot/compose/fallback.py:24`

## `bot/compose/circumstance.py`

Imports: none

## `bot/compose/clause.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/compose/fit.py:23`

## `bot/compose/colour_on_the_body.py`

Imports: none

## `bot/compose/doubt.py`

Imports: `bot/__init__.py:21`, `bot/compose/__init__.py:21`

## `bot/compose/draft.py`

Imports: `bot/__init__.py:29`, `bot/compose/__init__.py:30`, `bot/compose/badnews.py:30`, `bot/compose/loss.py:32`, `bot/memory/__init__.py:29`

## `bot/compose/facts.py`

Imports: `bot/__init__.py:15`, `bot/memory/__init__.py:15`, `bot/memory/narrow.py:15`

## `bot/compose/fallback.py`

Imports: `bot/__init__.py:15`, `bot/compose/__init__.py:23`, `bot/compose/badnews.py:25`, `bot/compose/circumstance.py:25`, `bot/compose/draft.py:25`, `bot/compose/fallback_chain.py:16851`, `bot/compose/fallback_data.py:46`, `bot/compose/fallback_pools.py:11533`, `bot/compose/fallback_words.py:3756`, `bot/compose/finding_out.py:25`, `bot/compose/goodnews.py:25`, `bot/compose/handoff.py:25`, `bot/compose/harm.py:25`, `bot/compose/leaving.py:25`, `bot/compose/loss.py:25`, `bot/compose/news_ask.py:10606`, `bot/compose/no_eyes.py:25`, `bot/compose/not_knowing.py:44`, `bot/compose/onething.py:25`, `bot/compose/outlook.py:25`, `bot/compose/place_facts.py:10577`, `bot/compose/postcode.py:28`, `bot/compose/prices.py:25`, `bot/compose/quantity.py:25`, `bot/compose/reaction.py:25`, `bot/compose/remark.py:25`, `bot/compose/stepwords.py:25`, `bot/compose/the_rule_for_it.py:12087`, `bot/compose/trouble.py:25`, `bot/compose/unseen.py:25`, `bot/compose/urgent.py:29`, `bot/compose/verdict.py:12439`, `bot/compose/why.py:25`, `bot/compose/worn.py:25`, `bot/knowledge/__init__.py:10161`, `bot/knowledge/sport.py:10161`, `bot/memory/__init__.py:15`, `bot/memory/ack.py:15`, `bot/memory/facts.py:15`, `bot/memory/history.py:15`, `bot/memory/more.py:13153`, `bot/memory/obstacle.py:15`, `bot/memory/reject.py:15`, `bot/memory/repeat.py:15`, `bot/memory/stuck.py:16`, `bot/memory/swap.py:15`, `bot/memory/topics.py:11325`, `bot/modules/__init__.py:8058`, `bot/modules/answers.py:8112`, `bot/modules/memory_store.py:8058`, `bot/understanding/__init__.py:17`, `bot/understanding/body.py:18`, `bot/understanding/kinds.py:19`, `bot/understanding/topics.py:22`

## `bot/compose/fallback_chain.py`

Imports: `bot/__init__.py:11`, `bot/compose/__init__.py:20`, `bot/compose/badnews.py:23`, `bot/compose/childwords.py:3014`, `bot/compose/circumstance.py:23`, `bot/compose/draft.py:23`, `bot/compose/fallback.py:757`, `bot/compose/fallback_data.py:41`, `bot/compose/fallback_pools.py:3972`, `bot/compose/finding_out.py:23`, `bot/compose/goodnews.py:23`, `bot/compose/handoff.py:23`, `bot/compose/harm.py:23`, `bot/compose/leaving.py:23`, `bot/compose/limit.py:22`, `bot/compose/loss.py:23`, `bot/compose/morning_after.py:23`, `bot/compose/no_eyes.py:23`, `bot/compose/onething.py:23`, `bot/compose/postcode.py:26`, `bot/compose/prices.py:23`, `bot/compose/quantity.py:23`, `bot/compose/reaction.py:23`, `bot/compose/reading.py:6594`, `bot/compose/remark.py:23`, `bot/compose/safetyask.py:23`, `bot/compose/talktime.py:3022`, `bot/compose/toknow.py:4608`, `bot/compose/trouble.py:23`, `bot/compose/unseen.py:23`, `bot/compose/urgent.py:27`, `bot/compose/verdict.py:2362`, `bot/compose/why.py:23`, `bot/compose/worn.py:23`, `bot/knowledge/__init__.py:2966`, `bot/knowledge/maths.py:2966`, `bot/memory/__init__.py:11`, `bot/memory/ack.py:11`, `bot/memory/facts.py:11`, `bot/memory/history.py:11`, `bot/memory/obstacle.py:11`, `bot/memory/reject.py:11`, `bot/memory/repeat.py:11`, `bot/memory/stuck.py:12`, `bot/memory/swap.py:11`, `bot/memory/topics.py:2960`, `bot/modules/__init__.py:1414`, `bot/modules/memory_store.py:1414`, `bot/understanding/__init__.py:13`, `bot/understanding/body.py:15`, `bot/understanding/kinds.py:14`, `bot/understanding/topics.py:19`

## `bot/compose/fallback_data.py`

Imports: `bot/__init__.py:11`, `bot/compose/__init__.py:11`, `bot/compose/finding_out.py:11`

## `bot/compose/fallback_pools.py`

Imports: `bot/__init__.py:16`, `bot/compose/__init__.py:16`, `bot/compose/fallback.py:22`, `bot/compose/fallback_chain.py:688`, `bot/compose/fallback_data.py:17`, `bot/compose/findout.py:16`, `bot/compose/for_the_day.py:16`, `bot/compose/own_ask.py:665`, `bot/compose/the_rule_for_it.py:21`, `bot/compose/waited_for.py:16`

## `bot/compose/fallback_words.py`

Imports: none

## `bot/compose/find_a_place.py`

Imports: `bot/__init__.py:40`, `bot/compose/__init__.py:40`, `bot/compose/fallback_data.py:40`, `bot/compose/finding_out.py:40`

## `bot/compose/finding_out.py`

Imports: none

## `bot/compose/findout.py`

Imports: `bot/__init__.py:26`, `bot/compose/__init__.py:26`, `bot/compose/fallback_data.py:662`

## `bot/compose/fit.py`

Imports: none

## `bot/compose/for_the_day.py`

Imports: none

## `bot/compose/gap.py`

Imports: none

## `bot/compose/goodnews.py`

Imports: `bot/__init__.py:18`, `bot/compose/__init__.py:18`, `bot/compose/badnews.py:19`

## `bot/compose/griefplea.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/compose/fallback.py:23`, `bot/compose/loss.py:23`

## `bot/compose/habit.py`

Imports: `bot/__init__.py:45`, `bot/compose/__init__.py:45`

## `bot/compose/handoff.py`

Imports: `bot/__init__.py:21`, `bot/compose/__init__.py:21`

## `bot/compose/hands.py`

Imports: `bot/__init__.py:44`, `bot/compose/__init__.py:44`, `bot/compose/fallback.py:253`

## `bot/compose/harm.py`

Imports: `bot/__init__.py:32`, `bot/compose/__init__.py:32`

## `bot/compose/hedge.py`

Imports: `bot/__init__.py:19`, `bot/compose/__init__.py:19`, `bot/compose/fallback.py:19`

## `bot/compose/how_it_went.py`

Imports: `bot/__init__.py:36`, `bot/compose/__init__.py:36`

## `bot/compose/invitation.py`

Imports: `bot/__init__.py:24`, `bot/compose/__init__.py:24`

## `bot/compose/is_it_worth_it.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/compose/fallback.py:23`

## `bot/compose/judgement.py`

Imports: `bot/__init__.py:18`, `bot/compose/__init__.py:18`, `bot/compose/fallback.py:19`, `bot/compose/fallback_data.py:18`

## `bot/compose/kept_nearby.py`

Imports: `bot/__init__.py:31`, `bot/compose/__init__.py:31`, `bot/compose/fallback_data.py:31`

## `bot/compose/lead.py`

Imports: `bot/__init__.py:31`, `bot/compose/__init__.py:31`, `bot/compose/fit.py:31`

## `bot/compose/lead_in.py`

Imports: `bot/__init__.py:44`, `bot/compose/__init__.py:44`, `bot/compose/fallback.py:44`, `bot/memory/__init__.py:45`, `bot/memory/ack.py:45`, `bot/modules/__init__.py:122`, `bot/modules/understand.py:122`

## `bot/compose/leaving.py`

Imports: none

## `bot/compose/limit.py`

Imports: none

## `bot/compose/liveask.py`

Imports: `bot/__init__.py:33`, `bot/compose/__init__.py:33`, `bot/compose/fallback.py:33`

## `bot/compose/loss.py`

Imports: `bot/__init__.py:26`, `bot/knowledge/__init__.py:4696`, `bot/knowledge/lookup.py:4696`, `bot/memory/__init__.py:22`, `bot/memory/dream.py:22`, `bot/understanding/__init__.py:26`, `bot/understanding/topics.py:27`

## `bot/compose/missing_half.py`

Imports: none

## `bot/compose/morning_after.py`

Imports: none

## `bot/compose/moving.py`

Imports: `bot/__init__.py:26`, `bot/compose/__init__.py:26`, `bot/compose/fallback.py:26`, `bot/compose/goodnews.py:27`

## `bot/compose/nearby_one.py`

Imports: none

## `bot/compose/news_ask.py`

Imports: none

## `bot/compose/no_advice.py`

Imports: `bot/__init__.py:247`, `bot/compose/__init__.py:247`

## `bot/compose/no_eyes.py`

Imports: `bot/__init__.py:486`, `bot/compose/__init__.py:486`, `bot/compose/fallback.py:486`

## `bot/compose/no_page.py`

Imports: none

## `bot/compose/not_knowing.py`

Imports: `bot/__init__.py:24`, `bot/compose/__init__.py:24`, `bot/compose/fallback_chain.py:33`, `bot/compose/fallback_data.py:24`, `bot/compose/routes.py:532`

## `bot/compose/nottheirbest.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`, `bot/compose/fallback.py:103`, `bot/compose/fallback_data.py:23`

## `bot/compose/occasionplan.py`

Imports: none

## `bot/compose/onething.py`

Imports: none

## `bot/compose/only_they_know.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:28`, `bot/compose/fallback.py:28`

## `bot/compose/onmybehalf.py`

Imports: `bot/__init__.py:40`, `bot/compose/__init__.py:40`, `bot/compose/fallback.py:214`, `bot/compose/fallback_data.py:40`, `bot/memory/__init__.py:215`, `bot/memory/store_lines.py:215`

## `bot/compose/onscreen.py`

Imports: `bot/__init__.py:31`, `bot/compose/__init__.py:32`, `bot/compose/fallback.py:32`, `bot/compose/verdict.py:32`, `bot/memory/__init__.py:303`, `bot/memory/history.py:303`, `bot/understanding/__init__.py:31`, `bot/understanding/kinds.py:31`

## `bot/compose/onsubject.py`

Imports: `bot/__init__.py:662`, `bot/compose/__init__.py:662`, `bot/compose/fallback.py:662`, `bot/knowledge/__init__.py:698`, `bot/knowledge/lookup.py:698`, `bot/modules/__init__.py:656`, `bot/modules/answers.py:656`, `bot/understanding/__init__.py:699`

## `bot/compose/outlook.py`

Imports: `bot/__init__.py:66`, `bot/compose/__init__.py:66`, `bot/compose/fallback_data.py:66`

## `bot/compose/own_ask.py`

Imports: none

## `bot/compose/own_facts.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`

## `bot/compose/pace.py`

Imports: `bot/__init__.py:24`, `bot/compose/__init__.py:24`

## `bot/compose/place_facts.py`

Imports: none

## `bot/compose/plan_ask.py`

Imports: none

## `bot/compose/plans.py`

Imports: none

## `bot/compose/pleased.py`

Imports: `bot/__init__.py:27`, `bot/compose/__init__.py:27`

## `bot/compose/postcode.py`

Imports: none

## `bot/compose/prices.py`

Imports: none

## `bot/compose/quantity.py`

Imports: `bot/__init__.py:43`, `bot/compose/__init__.py:44`, `bot/compose/prices.py:894`, `bot/memory/__init__.py:43`, `bot/understanding/__init__.py:778`, `bot/understanding/topics.py:778`

## `bot/compose/reaction.py`

Imports: `bot/__init__.py:239`, `bot/compose/__init__.py:239`, `bot/compose/fallback.py:239`

## `bot/compose/reading.py`

Imports: `bot/__init__.py:17`, `bot/compose/__init__.py:18`, `bot/compose/verdict.py:18`, `bot/knowledge/__init__.py:1541`, `bot/knowledge/lookup.py:1541`, `bot/memory/__init__.py:17`, `bot/memory/price.py:1847`, `bot/memory/topics.py:17`

## `bot/compose/recall.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:28`

## `bot/compose/receiving_end.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`, `bot/compose/fallback.py:22`

## `bot/compose/remark.py`

Imports: `bot/__init__.py:31`, `bot/compose/__init__.py:31`, `bot/compose/fallback_data.py:32`, `bot/compose/trouble.py:31`, `bot/memory/__init__.py:33`, `bot/memory/facts.py:33`, `bot/memory/repeat.py:2195`, `bot/modules/__init__.py:588`, `bot/modules/answers.py:588`, `bot/understanding/__init__.py:34`, `bot/understanding/kinds.py:34`, `bot/understanding/normalize.py:35`

## `bot/compose/repair.py`

Imports: none

## `bot/compose/rotten_luck.py`

Imports: `bot/__init__.py:33`, `bot/compose/__init__.py:33`

## `bot/compose/routes.py`

Imports: none

## `bot/compose/rule.py`

Imports: `bot/__init__.py:36`, `bot/compose/__init__.py:36`

## `bot/compose/ruledout.py`

Imports: `bot/__init__.py:18`, `bot/compose/__init__.py:18`, `bot/compose/fallback.py:18`, `bot/memory/__init__.py:566`, `bot/memory/repeat.py:566`, `bot/understanding/__init__.py:23`, `bot/understanding/kinds.py:23`

## `bot/compose/safety_doubt.py`

Imports: `bot/__init__.py:34`, `bot/compose/__init__.py:34`, `bot/compose/fallback.py:34`

## `bot/compose/safetyask.py`

Imports: `bot/__init__.py:34`, `bot/memory/__init__.py:34`, `bot/modules/__init__.py:210`, `bot/modules/answers.py:210`

## `bot/compose/said_about_me.py`

Imports: `bot/__init__.py:26`, `bot/compose/__init__.py:26`

## `bot/compose/saidby.py`

Imports: `bot/__init__.py:44`, `bot/compose/__init__.py:44`, `bot/modules/__init__.py:213`, `bot/modules/answers.py:213`

## `bot/compose/setting_off.py`

Imports: none

## `bot/compose/stepwords.py`

Imports: none

## `bot/compose/still.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:28`, `bot/compose/fallback.py:28`, `bot/compose/verdict.py:28`, `bot/memory/__init__.py:29`, `bot/memory/another.py:29`, `bot/memory/repeat.py:215`

## `bot/compose/stretches.py`

Imports: `bot/__init__.py:25`, `bot/compose/__init__.py:25`, `bot/compose/fallback.py:25`, `bot/compose/fallback_data.py:25`, `bot/memory/__init__.py:26`, `bot/memory/ack.py:26`

## `bot/compose/talktime.py`

Imports: `bot/__init__.py:29`, `bot/compose/__init__.py:29`, `bot/compose/fallback.py:29`

## `bot/compose/talkto.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`

## `bot/compose/taste.py`

Imports: `bot/__init__.py:49`, `bot/compose/__init__.py:49`, `bot/compose/fallback.py:49`

## `bot/compose/the_course_of_it.py`

Imports: `bot/__init__.py:337`, `bot/compose/__init__.py:364`, `bot/compose/fallback.py:364`, `bot/knowledge/__init__.py:338`, `bot/knowledge/lookup.py:338`, `bot/understanding/__init__.py:337`

## `bot/compose/the_rule_for_it.py`

Imports: none

## `bot/compose/the_shape_without_the_fact.py`

Imports: `bot/__init__.py:623`, `bot/compose/__init__.py:623`, `bot/compose/fallback.py:623`

## `bot/compose/the_trouble_named.py`

Imports: `bot/__init__.py:26`, `bot/compose/__init__.py:26`, `bot/compose/fallback.py:26`

## `bot/compose/the_turn_after.py`

Imports: `bot/__init__.py:39`, `bot/compose/__init__.py:39`, `bot/compose/fallback.py:39`, `bot/compose/fallback_data.py:39`

## `bot/compose/their_day.py`

Imports: `bot/__init__.py:40`, `bot/compose/__init__.py:40`, `bot/compose/fallback_data.py:193`, `bot/understanding/__init__.py:214`

## `bot/compose/their_detail.py`

Imports: `bot/__init__.py:33`, `bot/compose/__init__.py:33`, `bot/compose/fallback.py:33`, `bot/modules/__init__.py:172`, `bot/modules/answers.py:172`

## `bot/compose/their_life.py`

Imports: none

## `bot/compose/theirperson.py`

Imports: `bot/__init__.py:38`, `bot/compose/__init__.py:38`, `bot/compose/fallback.py:271`

## `bot/compose/thread_detail.py`

Imports: `bot/__init__.py:29`, `bot/compose/__init__.py:29`, `bot/compose/aboutness.py:29`, `bot/compose/fallback.py:30`

## `bot/compose/toknow.py`

Imports: `bot/__init__.py:33`, `bot/compose/__init__.py:33`, `bot/modules/__init__.py:105`, `bot/modules/answers.py:105`

## `bot/compose/tone_again.py`

Imports: none

## `bot/compose/trouble.py`

Imports: `bot/__init__.py:36`, `bot/compose/__init__.py:36`, `bot/compose/badnews.py:36`, `bot/knowledge/__init__.py:771`, `bot/knowledge/lookup.py:771`, `bot/memory/__init__.py:37`, `bot/memory/setback.py:37`, `bot/modules/__init__.py:752`, `bot/modules/chat.py:752`

## `bot/compose/unseen.py`

Imports: none

## `bot/compose/urgent.py`

Imports: `bot/__init__.py:21`, `bot/memory/__init__.py:21`, `bot/memory/dream.py:21`

## `bot/compose/verdict.py`

Imports: `bot/__init__.py:27`, `bot/compose/__init__.py:29`, `bot/compose/badnews.py:2328`, `bot/compose/fallback.py:2123`, `bot/compose/fallback_data.py:860`, `bot/compose/goodnews.py:32`, `bot/compose/loss.py:36`, `bot/compose/safetyask.py:5440`, `bot/compose/why.py:29`, `bot/memory/__init__.py:27`, `bot/memory/ack.py:2084`, `bot/memory/again.py:27`, `bot/memory/healing.py:30`, `bot/memory/topics.py:3761`, `bot/modules/__init__.py:4222`, `bot/modules/chat.py:4222`, `bot/understanding/__init__.py:28`, `bot/understanding/normalize.py:28`

## `bot/compose/waited_for.py`

Imports: `bot/__init__.py:16`, `bot/memory/__init__.py:16`, `bot/memory/holds.py:16`

## `bot/compose/ward.py`

Imports: none

## `bot/compose/what_decides_it.py`

Imports: none

## `bot/compose/what_i_do_know.py`

Imports: `bot/__init__.py:42`, `bot/compose/__init__.py:42`, `bot/compose/fallback.py:42`

## `bot/compose/what_it_asks.py`

Imports: `bot/__init__.py:32`, `bot/compose/__init__.py:32`, `bot/compose/fit.py:32`

## `bot/compose/who.py`

Imports: `bot/__init__.py:26`, `bot/compose/__init__.py:29`, `bot/memory/__init__.py:26`, `bot/memory/holds.py:26`, `bot/understanding/__init__.py:27`

## `bot/compose/who_to_see.py`

Imports: `bot/__init__.py:32`, `bot/compose/__init__.py:32`, `bot/compose/verdict.py:32`

## `bot/compose/whole.py`

Imports: none

## `bot/compose/why.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/memory/__init__.py:24`, `bot/memory/again.py:24`, `bot/memory/another.py:24`, `bot/memory/topics.py:2089`

## `bot/compose/worn.py`

Imports: `bot/__init__.py:35`, `bot/compose/__init__.py:35`, `bot/compose/trouble.py:35`

## `bot/knowledge/__init__.py`

Imports: none

## `bot/knowledge/a_plain_noise_word_on_a_machine.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/affording.py`

Imports: `bot/__init__.py:19`, `bot/knowledge/__init__.py:19`, `bot/knowledge/lookup.py:19`

## `bot/knowledge/animals.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/appointments.py`

Imports: `bot/__init__.py:11`, `bot/knowledge/__init__.py:11`, `bot/knowledge/lookup.py:11`, `bot/memory/__init__.py:129`

## `bot/knowledge/babyfood.py`

Imports: `bot/__init__.py:49`, `bot/knowledge/__init__.py:49`, `bot/knowledge/lookup.py:49`

## `bot/knowledge/bake_faults.py`

Imports: none

## `bot/knowledge/bikes.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/body.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/lookup.py:16`, `bot/knowledge/pets.py:2096`, `bot/understanding/__init__.py:17`, `bot/understanding/kinds.py:17`

## `bot/knowledge/body_energy.py`

Imports: `bot/__init__.py:20`, `bot/knowledge/__init__.py:20`, `bot/knowledge/lookup.py:20`

## `bot/knowledge/build_jobs.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/lookup.py:16`

## `bot/knowledge/business.py`

Imports: `bot/__init__.py:20`, `bot/knowledge/__init__.py:20`, `bot/knowledge/lookup.py:20`, `bot/memory/__init__.py:107`, `bot/memory/repeat.py:107`

## `bot/knowledge/cakes.py`

Imports: none

## `bot/knowledge/caring.py`

Imports: `bot/__init__.py:23`, `bot/knowledge/__init__.py:23`, `bot/knowledge/lookup.py:23`

## `bot/knowledge/cars.py`

Imports: `bot/__init__.py:12`, `bot/knowledge/__init__.py:14`, `bot/knowledge/costs.py:4094`, `bot/knowledge/lookup.py:14`, `bot/understanding/__init__.py:12`, `bot/understanding/car_glass.py:12`, `bot/understanding/cars.py:13`

## `bot/knowledge/child_work.py`

Imports: `bot/__init__.py:14`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`, `bot/understanding/__init__.py:14`, `bot/understanding/lexicon.py:14`

## `bot/knowledge/chilli.py`

Imports: `bot/__init__.py:12`, `bot/knowledge/__init__.py:12`, `bot/knowledge/cooking_entries.py:74`, `bot/knowledge/dishes.py:59`, `bot/knowledge/lookup.py:12`

## `bot/knowledge/cleaning.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/clothing.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/home.py:1036`, `bot/knowledge/lookup.py:16`, `bot/knowledge/wording.py:15`, `bot/memory/__init__.py:557`, `bot/memory/thread.py:797`

## `bot/knowledge/coats.py`

Imports: `bot/__init__.py:22`, `bot/knowledge/__init__.py:22`, `bot/knowledge/lookup.py:22`

## `bot/knowledge/comeback.py`

Imports: `bot/__init__.py:26`, `bot/knowledge/__init__.py:26`, `bot/knowledge/lookup.py:26`

## `bot/knowledge/complaints.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/health.py:469`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/conditions.py`

Imports: `bot/__init__.py:22`, `bot/knowledge/__init__.py:22`, `bot/knowledge/lookup.py:22`

## `bot/knowledge/consumer.py`

Imports: `bot/__init__.py:12`, `bot/knowledge/__init__.py:12`, `bot/knowledge/cooking.py:568`, `bot/knowledge/lookup.py:12`

## `bot/knowledge/containers_frost.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/cook_kit.py`

Imports: `bot/__init__.py:27`, `bot/knowledge/__init__.py:27`, `bot/knowledge/cooking.py:27`, `bot/knowledge/lookup.py:30`

## `bot/knowledge/cooking.py`

Imports: `bot/__init__.py:19`, `bot/knowledge/__init__.py:20`, `bot/knowledge/bake_faults.py:27`, `bot/knowledge/cooking_entries.py:9185`, `bot/knowledge/cooking_more.py:12883`, `bot/knowledge/cooking_tables.py:31`, `bot/knowledge/dishes.py:20`, `bot/knowledge/garden.py:695`, `bot/knowledge/keeping_cooked.py:21`, `bot/knowledge/lookup.py:22`, `bot/knowledge/meals.py:24`, `bot/knowledge/picking.py:11484`, `bot/knowledge/prices.py:29`, `bot/knowledge/units.py:30`, `bot/memory/__init__.py:6234`, `bot/modules/__init__.py:3892`, `bot/modules/memory_store.py:3892`, `bot/understanding/__init__.py:19`

## `bot/knowledge/cooking_entries.py`

Imports: `bot/__init__.py:11`, `bot/compose/__init__.py:11`, `bot/compose/repair.py:11`, `bot/knowledge/__init__.py:12`, `bot/knowledge/body.py:18`, `bot/knowledge/cooking.py:20`, `bot/knowledge/dishes.py:12`, `bot/knowledge/freshness.py:13`, `bot/knowledge/keeping.py:837`, `bot/knowledge/lookup.py:15`, `bot/knowledge/meals.py:2029`, `bot/knowledge/shopping.py:17`, `bot/memory/__init__.py:11965`

## `bot/knowledge/cooking_fats.py`

Imports: `bot/__init__.py:14`, `bot/compose/__init__.py:14`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/cooking_gaps.py`

Imports: `bot/__init__.py:40`, `bot/knowledge/__init__.py:40`, `bot/knowledge/cooking.py:42`, `bot/knowledge/dishes.py:40`, `bot/knowledge/lookup.py:41`

## `bot/knowledge/cooking_more.py`

Imports: `bot/__init__.py:20`, `bot/knowledge/__init__.py:20`, `bot/knowledge/cooking.py:23`, `bot/knowledge/lookup.py:20`

## `bot/knowledge/cooking_tables.py`

Imports: none

## `bot/knowledge/cooling.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/costs.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/lookup.py:16`

## `bot/knowledge/creative.py`

Imports: `bot/__init__.py:14`, `bot/knowledge/__init__.py:14`, `bot/knowledge/lookup.py:14`

## `bot/knowledge/crisis_replies.py`

Imports: none

## `bot/knowledge/dates.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/lookup.py:17`

## `bot/knowledge/decorating.py`

Imports: `bot/__init__.py:34`, `bot/knowledge/__init__.py:34`, `bot/knowledge/lookup.py:34`

## `bot/knowledge/desk_setup.py`

Imports: `bot/__init__.py:8`, `bot/knowledge/__init__.py:8`, `bot/knowledge/lookup.py:8`

## `bot/knowledge/devices.py`

Imports: `bot/__init__.py:19`, `bot/knowledge/__init__.py:19`, `bot/knowledge/lookup.py:19`

## `bot/knowledge/differences.py`

Imports: `bot/__init__.py:31`, `bot/knowledge/__init__.py:31`, `bot/knowledge/lookup.py:31`

## `bot/knowledge/dish_verdicts.py`

Imports: `bot/__init__.py:33`, `bot/knowledge/__init__.py:33`, `bot/knowledge/lookup.py:33`, `bot/knowledge/nutrition.py:40`

## `bot/knowledge/dishes.py`

Imports: `bot/__init__.py:32`, `bot/knowledge/__init__.py:32`, `bot/knowledge/cakes.py:32`, `bot/knowledge/cleaning.py:34`, `bot/knowledge/cooking.py:2543`, `bot/knowledge/lookup.py:35`, `bot/knowledge/shopping.py:37`, `bot/understanding/__init__.py:38`, `bot/understanding/topics.py:38`

## `bot/knowledge/drinks.py`

Imports: `bot/__init__.py:23`, `bot/knowledge/__init__.py:23`, `bot/knowledge/lookup.py:23`

## `bot/knowledge/eithers.py`

Imports: `bot/__init__.py:25`, `bot/knowledge/__init__.py:25`, `bot/knowledge/opinions.py:25`

## `bot/knowledge/entertainment.py`

Imports: `bot/__init__.py:9`, `bot/compose/__init__.py:9`, `bot/compose/repair.py:9`, `bot/knowledge/__init__.py:11`, `bot/knowledge/cooking.py:2236`, `bot/knowledge/kept_animals.py:10445`, `bot/knowledge/lookup.py:11`, `bot/knowledge/travel.py:45`, `bot/understanding/__init__.py:10`, `bot/understanding/attempts.py:10`

## `bot/knowledge/everyday_lines.py`

Imports: `bot/__init__.py:10`, `bot/knowledge/__init__.py:10`, `bot/knowledge/lookup.py:10`

## `bot/knowledge/explains.py`

Imports: `bot/__init__.py:20`, `bot/knowledge/__init__.py:20`, `bot/knowledge/lookup.py:20`

## `bot/knowledge/fabrics.py`

Imports: `bot/__init__.py:19`, `bot/knowledge/__init__.py:19`, `bot/knowledge/lookup.py:19`

## `bot/knowledge/family.py`

Imports: `bot/__init__.py:18`, `bot/compose/__init__.py:20`, `bot/compose/repair.py:20`, `bot/knowledge/__init__.py:18`, `bot/knowledge/clothing.py:19`, `bot/knowledge/cooking.py:2383`, `bot/knowledge/family_more.py:13984`, `bot/knowledge/lookup.py:21`, `bot/knowledge/pets.py:18`, `bot/knowledge/wording.py:24`, `bot/understanding/__init__.py:22`, `bot/understanding/kinds.py:22`, `bot/understanding/normalize.py:23`

## `bot/knowledge/family_more.py`

Imports: `bot/__init__.py:20`, `bot/knowledge/__init__.py:20`, `bot/knowledge/family.py:21`, `bot/knowledge/geography.py:55`, `bot/knowledge/lookup.py:20`, `bot/memory/__init__.py:790`

## `bot/knowledge/fears.py`

Imports: `bot/__init__.py:22`, `bot/knowledge/__init__.py:22`, `bot/knowledge/lookup.py:22`

## `bot/knowledge/fitness.py`

Imports: `bot/__init__.py:13`, `bot/knowledge/__init__.py:13`, `bot/knowledge/body.py:14`, `bot/knowledge/lookup.py:13`

## `bot/knowledge/fitweek.py`

Imports: `bot/__init__.py:13`, `bot/knowledge/__init__.py:13`, `bot/knowledge/home.py:18`, `bot/knowledge/lookup.py:13`

## `bot/knowledge/food_left_out.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/lookup.py:16`

## `bot/knowledge/freshness.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`, `bot/memory/__init__.py:533`

## `bot/knowledge/friends.py`

Imports: `bot/__init__.py:9`, `bot/compose/__init__.py:9`, `bot/compose/repair.py:9`, `bot/knowledge/__init__.py:10`, `bot/knowledge/dishes.py:10`, `bot/knowledge/lookup.py:11`, `bot/knowledge/wording.py:12`

## `bot/knowledge/fruit.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/lookup.py:16`

## `bot/knowledge/fun.py`

Imports: `bot/__init__.py:20`, `bot/compose/__init__.py:20`, `bot/compose/loss.py:21`, `bot/knowledge/__init__.py:22`, `bot/knowledge/lookup.py:22`, `bot/knowledge/pets.py:23`

## `bot/knowledge/garden.py`

Imports: `bot/__init__.py:10`, `bot/knowledge/__init__.py:10`, `bot/knowledge/lookup.py:10`

## `bot/knowledge/garden_tools.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/geography.py`

Imports: `bot/__init__.py:9`, `bot/knowledge/__init__.py:9`, `bot/knowledge/lookup.py:9`

## `bot/knowledge/habits.py`

Imports: `bot/__init__.py:20`, `bot/compose/__init__.py:145`, `bot/compose/repair.py:145`, `bot/knowledge/__init__.py:20`, `bot/knowledge/lookup.py:20`, `bot/knowledge/wording.py:21`

## `bot/knowledge/health.py`

Imports: `bot/__init__.py:10`, `bot/knowledge/__init__.py:10`, `bot/knowledge/body.py:13`, `bot/knowledge/costs.py:8286`, `bot/knowledge/lookup.py:11`, `bot/knowledge/pets.py:10`, `bot/knowledge/shopping.py:14`

## `bot/knowledge/history.py`

Imports: `bot/__init__.py:11`, `bot/knowledge/__init__.py:11`, `bot/knowledge/lookup.py:11`

## `bot/knowledge/home.py`

Imports: `bot/__init__.py:11`, `bot/compose/__init__.py:11`, `bot/knowledge/__init__.py:12`, `bot/knowledge/body.py:14`, `bot/knowledge/dishes.py:15`, `bot/knowledge/home_craft.py:8266`, `bot/knowledge/lookup.py:12`, `bot/knowledge/travel.py:4242`, `bot/knowledge/wording.py:13`, `bot/memory/__init__.py:3749`, `bot/memory/topics.py:3749`

## `bot/knowledge/home_craft.py`

Imports: `bot/__init__.py:32`, `bot/knowledge/__init__.py:33`, `bot/knowledge/body.py:529`, `bot/knowledge/costs.py:33`, `bot/knowledge/home.py:656`, `bot/knowledge/lookup.py:915`, `bot/understanding/__init__.py:32`, `bot/understanding/topics.py:32`

## `bot/knowledge/house_odds.py`

Imports: `bot/__init__.py:35`, `bot/knowledge/__init__.py:35`, `bot/knowledge/lookup.py:35`

## `bot/knowledge/how_it_works.py`

Imports: `bot/__init__.py:47`, `bot/knowledge/__init__.py:47`, `bot/knowledge/lookup.py:47`

## `bot/knowledge/icing.py`

Imports: `bot/__init__.py:26`, `bot/knowledge/__init__.py:26`, `bot/knowledge/lookup.py:26`

## `bot/knowledge/idioms.py`

Imports: `bot/__init__.py:24`, `bot/knowledge/__init__.py:24`, `bot/knowledge/lookup.py:24`

## `bot/knowledge/income_periods.py`

Imports: `bot/__init__.py:14`, `bot/knowledge/__init__.py:14`, `bot/knowledge/lookup.py:14`, `bot/knowledge/money.py:15`

## `bot/knowledge/ingredient_swaps.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/lookup.py:17`

## `bot/knowledge/job_times.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/costs.py:15`

## `bot/knowledge/joint_time.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/cooking.py:18`, `bot/knowledge/lookup.py:19`

## `bot/knowledge/joints.py`

Imports: `bot/__init__.py:21`, `bot/knowledge/__init__.py:21`, `bot/knowledge/lookup.py:21`

## `bot/knowledge/keeping.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/cooking.py:17`, `bot/knowledge/dishes.py:18`, `bot/knowledge/lookup.py:19`

## `bot/knowledge/keeping_cooked.py`

Imports: none

## `bot/knowledge/kept_animals.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/lookup.py:16`

## `bot/knowledge/kit_lists.py`

Imports: `bot/__init__.py:11`, `bot/knowledge/__init__.py:11`, `bot/knowledge/lookup.py:11`

## `bot/knowledge/kitchen.py`

Imports: `bot/__init__.py:14`, `bot/knowledge/__init__.py:14`, `bot/knowledge/lookup.py:14`

## `bot/knowledge/kitchen_odds.py`

Imports: `bot/__init__.py:27`, `bot/knowledge/__init__.py:27`, `bot/knowledge/body.py:27`, `bot/knowledge/dishes.py:104`, `bot/knowledge/lookup.py:28`

## `bot/knowledge/kitchen_prep.py`

Imports: `bot/__init__.py:28`, `bot/knowledge/__init__.py:29`, `bot/knowledge/lookup.py:29`, `bot/memory/__init__.py:28`

## `bot/knowledge/kits_and_claims.py`

Imports: `bot/__init__.py:19`, `bot/knowledge/__init__.py:19`, `bot/knowledge/lookup.py:19`

## `bot/knowledge/land.py`

Imports: `bot/__init__.py:12`, `bot/knowledge/__init__.py:12`, `bot/knowledge/lookup.py:12`

## `bot/knowledge/language.py`

Imports: `bot/__init__.py:17`, `bot/compose/__init__.py:3159`, `bot/compose/draft.py:3159`, `bot/knowledge/__init__.py:17`, `bot/knowledge/lookup.py:17`, `bot/knowledge/pets.py:19`, `bot/knowledge/wording.py:22`

## `bot/knowledge/left_on.py`

Imports: `bot/__init__.py:22`, `bot/knowledge/__init__.py:22`, `bot/knowledge/lookup.py:22`

## `bot/knowledge/lifeadmin.py`

Imports: `bot/__init__.py:22`, `bot/knowledge/__init__.py:22`, `bot/knowledge/costs.py:4218`, `bot/knowledge/health.py:22`, `bot/knowledge/lookup.py:23`, `bot/knowledge/technology.py:24`

## `bot/knowledge/lookup.py`

Imports: `bot/__init__.py:44`, `bot/knowledge/__init__.py:5037`, `bot/knowledge/prices.py:5058`, `bot/memory/__init__.py:31`, `bot/memory/dream.py:31`, `bot/understanding/__init__.py:44`, `bot/understanding/lexicon.py:44`, `bot/understanding/normalize.py:45`

## `bot/knowledge/machine_door.py`

Imports: `bot/__init__.py:9`, `bot/knowledge/__init__.py:9`, `bot/knowledge/lookup.py:9`

## `bot/knowledge/materials.py`

Imports: `bot/__init__.py:23`, `bot/knowledge/__init__.py:23`, `bot/knowledge/lookup.py:23`

## `bot/knowledge/maths.py`

Imports: `bot/__init__.py:20`, `bot/knowledge/__init__.py:22`, `bot/knowledge/lookup.py:23`, `bot/knowledge/money.py:2386`, `bot/knowledge/units.py:22`, `bot/memory/__init__.py:20`, `bot/memory/saving.py:20`, `bot/memory/topics.py:21`

## `bot/knowledge/meals.py`

Imports: `bot/__init__.py:19`, `bot/compose/__init__.py:23`, `bot/compose/fit.py:23`, `bot/knowledge/__init__.py:19`, `bot/knowledge/cleaning.py:19`, `bot/knowledge/cooking.py:1719`, `bot/knowledge/dishes.py:21`, `bot/knowledge/lookup.py:20`, `bot/knowledge/occasions.py:22`, `bot/understanding/__init__.py:24`

## `bot/knowledge/measures.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/meat_in_a_card.py`

Imports: `bot/__init__.py:29`, `bot/knowledge/__init__.py:29`, `bot/knowledge/cooking.py:29`

## `bot/knowledge/microwave_methods.py`

Imports: `bot/__init__.py:19`, `bot/knowledge/__init__.py:19`, `bot/knowledge/lookup.py:19`

## `bot/knowledge/money.py`

Imports: `bot/__init__.py:13`, `bot/compose/__init__.py:13`, `bot/compose/repair.py:13`, `bot/knowledge/__init__.py:16`, `bot/knowledge/consumer.py:16`, `bot/knowledge/cooking.py:17`, `bot/knowledge/geography.py:18`, `bot/knowledge/lifeadmin.py:19`, `bot/knowledge/lookup.py:21`, `bot/knowledge/prices.py:20`, `bot/knowledge/technology.py:22`, `bot/memory/__init__.py:14`, `bot/memory/facts.py:15`, `bot/memory/history.py:14`

## `bot/knowledge/money_words.py`

Imports: `bot/__init__.py:13`, `bot/knowledge/__init__.py:13`, `bot/knowledge/lookup.py:13`

## `bot/knowledge/music.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/nature.py`

Imports: `bot/__init__.py:12`, `bot/knowledge/__init__.py:12`, `bot/knowledge/lookup.py:12`

## `bot/knowledge/newborn.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/family.py:172`, `bot/knowledge/lookup.py:17`

## `bot/knowledge/no_kit.py`

Imports: `bot/__init__.py:28`, `bot/knowledge/__init__.py:28`, `bot/knowledge/lookup.py:28`, `bot/knowledge/swaps.py:29`

## `bot/knowledge/notable.py`

Imports: `bot/__init__.py:32`, `bot/knowledge/__init__.py:32`, `bot/knowledge/lookup.py:32`

## `bot/knowledge/nursing.py`

Imports: `bot/__init__.py:21`, `bot/knowledge/__init__.py:21`, `bot/knowledge/lookup.py:21`, `bot/knowledge/study.py:22`

## `bot/knowledge/nutrition.py`

Imports: `bot/__init__.py:3`, `bot/knowledge/__init__.py:3`, `bot/knowledge/cooking.py:18`, `bot/knowledge/lookup.py:19`, `bot/knowledge/pets.py:3`, `bot/knowledge/shopping.py:21`

## `bot/knowledge/occasions.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/geography.py:16`, `bot/knowledge/lookup.py:17`

## `bot/knowledge/odours.py`

Imports: `bot/__init__.py:24`, `bot/knowledge/__init__.py:24`, `bot/knowledge/lookup.py:24`

## `bot/knowledge/opinions.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/entertainment.py:17`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/painting_faults.py`

Imports: `bot/__init__.py:29`, `bot/knowledge/__init__.py:29`, `bot/knowledge/lookup.py:29`

## `bot/knowledge/panic_talk.py`

Imports: `bot/__init__.py:23`, `bot/knowledge/__init__.py:23`, `bot/knowledge/lookup.py:23`

## `bot/knowledge/parking.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/lookup.py:17`

## `bot/knowledge/pasta.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/lookup.py:17`

## `bot/knowledge/pay.py`

Imports: `bot/__init__.py:25`, `bot/knowledge/__init__.py:25`, `bot/knowledge/lookup.py:25`

## `bot/knowledge/pensions.py`

Imports: `bot/__init__.py:14`, `bot/knowledge/__init__.py:14`, `bot/knowledge/lookup.py:14`

## `bot/knowledge/pets.py`

Imports: `bot/__init__.py:9`, `bot/knowledge/__init__.py:9`, `bot/knowledge/costs.py:10293`, `bot/knowledge/lookup.py:9`, `bot/memory/__init__.py:3076`, `bot/memory/named.py:3076`, `bot/memory/topics.py:4191`

## `bot/knowledge/pets_health.py`

Imports: `bot/__init__.py:20`, `bot/knowledge/__init__.py:20`, `bot/knowledge/lookup.py:20`

## `bot/knowledge/pharmacy.py`

Imports: `bot/__init__.py:13`, `bot/knowledge/__init__.py:14`, `bot/knowledge/lookup.py:14`, `bot/memory/__init__.py:13`

## `bot/knowledge/phone_use.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/lookup.py:17`

## `bot/knowledge/photography.py`

Imports: `bot/__init__.py:11`, `bot/knowledge/__init__.py:11`, `bot/knowledge/lookup.py:11`

## `bot/knowledge/picking.py`

Imports: `bot/__init__.py:24`, `bot/knowledge/__init__.py:24`, `bot/knowledge/lookup.py:24`

## `bot/knowledge/picks.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/entertainment.py:17`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/plans.py`

Imports: `bot/__init__.py:17`, `bot/compose/__init__.py:17`, `bot/compose/repair.py:18`, `bot/knowledge/__init__.py:19`, `bot/knowledge/cooking.py:19`, `bot/knowledge/dishes.py:20`, `bot/knowledge/lookup.py:21`, `bot/knowledge/travel.py:966`

## `bot/knowledge/power_heat.py`

Imports: `bot/__init__.py:11`, `bot/knowledge/__init__.py:11`, `bot/knowledge/lookup.py:11`

## `bot/knowledge/pregnancy.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/lookup.py:17`

## `bot/knowledge/prices.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/lookup.py:16`

## `bot/knowledge/procedures.py`

Imports: `bot/__init__.py:23`, `bot/knowledge/__init__.py:23`, `bot/knowledge/lookup.py:23`

## `bot/knowledge/quantity.py`

Imports: none

## `bot/knowledge/rice.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/lookup.py:16`

## `bot/knowledge/routines.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/body.py:18`, `bot/knowledge/lookup.py:20`, `bot/knowledge/wording.py:19`

## `bot/knowledge/rubbish.py`

Imports: `bot/__init__.py:23`, `bot/knowledge/__init__.py:23`, `bot/knowledge/dishes.py:23`, `bot/knowledge/lookup.py:24`

## `bot/knowledge/sauces.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/school_day.py`

Imports: `bot/__init__.py:22`, `bot/knowledge/__init__.py:22`, `bot/knowledge/lookup.py:22`

## `bot/knowledge/school_days.py`

Imports: `bot/__init__.py:11`, `bot/knowledge/__init__.py:11`, `bot/knowledge/lookup.py:11`

## `bot/knowledge/schoolmaths.py`

Imports: `bot/__init__.py:16`, `bot/knowledge/__init__.py:16`, `bot/knowledge/lookup.py:16`

## `bot/knowledge/science.py`

Imports: `bot/__init__.py:12`, `bot/compose/__init__.py:13`, `bot/compose/repair.py:13`, `bot/knowledge/__init__.py:12`, `bot/knowledge/body.py:2719`, `bot/knowledge/habits.py:51`, `bot/knowledge/health.py:12`, `bot/knowledge/lookup.py:14`, `bot/memory/__init__.py:540`, `bot/memory/thread.py:540`

## `bot/knowledge/seasoning.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/shopping.py`

Imports: `bot/__init__.py:8`, `bot/compose/__init__.py:8`, `bot/knowledge/__init__.py:11`, `bot/knowledge/lookup.py:11`, `bot/memory/__init__.py:9`, `bot/memory/newborn.py:9`, `bot/memory/people.py:9`, `bot/understanding/__init__.py:10`, `bot/understanding/topics.py:10`

## `bot/knowledge/sleep.py`

Imports: `bot/__init__.py:9`, `bot/knowledge/__init__.py:9`, `bot/knowledge/lookup.py:9`

## `bot/knowledge/sleep_aids.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/spice_heat.py`

Imports: `bot/__init__.py:14`, `bot/knowledge/__init__.py:14`, `bot/knowledge/lookup.py:14`

## `bot/knowledge/sport.py`

Imports: `bot/__init__.py:13`, `bot/knowledge/__init__.py:13`, `bot/knowledge/lookup.py:13`

## `bot/knowledge/still_ok.py`

Imports: `bot/__init__.py:31`, `bot/knowledge/__init__.py:31`, `bot/knowledge/cooking.py:31`, `bot/knowledge/lookup.py:32`, `bot/knowledge/meals.py:261`

## `bot/knowledge/study.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/lookup.py:19`, `bot/knowledge/quantity.py:18`, `bot/knowledge/wording.py:21`, `bot/understanding/__init__.py:20`, `bot/understanding/normalize.py:20`

## `bot/knowledge/swaps.py`

Imports: `bot/__init__.py:31`, `bot/knowledge/__init__.py:31`, `bot/knowledge/lookup.py:31`

## `bot/knowledge/takes.py`

Imports: `bot/__init__.py:33`, `bot/knowledge/__init__.py:33`, `bot/knowledge/lookup.py:33`

## `bot/knowledge/technology.py`

Imports: `bot/__init__.py:12`, `bot/knowledge/__init__.py:13`, `bot/knowledge/lookup.py:13`, `bot/memory/__init__.py:1097`, `bot/memory/topics.py:1097`, `bot/understanding/__init__.py:12`, `bot/understanding/topics.py:12`

## `bot/knowledge/tenancy.py`

Imports: `bot/__init__.py:27`, `bot/knowledge/__init__.py:27`, `bot/knowledge/lookup.py:27`, `bot/knowledge/money.py:69`, `bot/knowledge/work.py:69`

## `bot/knowledge/trades.py`

Imports: `bot/__init__.py:11`, `bot/knowledge/__init__.py:11`, `bot/knowledge/lookup.py:11`

## `bot/knowledge/travel.py`

Imports: `bot/__init__.py:12`, `bot/compose/__init__.py:15`, `bot/knowledge/__init__.py:13`, `bot/knowledge/geography.py:13`, `bot/knowledge/lifeadmin.py:255`, `bot/knowledge/lookup.py:16`, `bot/memory/__init__.py:12`, `bot/memory/topics.py:3046`, `bot/memory/when.py:12`

## `bot/knowledge/travel_health.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/units.py`

Imports: `bot/__init__.py:1435`, `bot/memory/__init__.py:1435`

## `bot/knowledge/vaccines.py`

Imports: `bot/__init__.py:17`, `bot/knowledge/__init__.py:17`, `bot/knowledge/health.py:17`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/vegetables.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/warm_food.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/water_out.py`

Imports: `bot/__init__.py:15`, `bot/knowledge/__init__.py:15`, `bot/knowledge/lookup.py:15`

## `bot/knowledge/what_damages_what.py`

Imports: `bot/__init__.py:19`, `bot/knowledge/__init__.py:19`, `bot/knowledge/lookup.py:19`

## `bot/knowledge/whenbuy.py`

Imports: `bot/__init__.py:18`, `bot/knowledge/__init__.py:18`, `bot/knowledge/lookup.py:18`

## `bot/knowledge/wording.py`

Imports: `bot/__init__.py:10`, `bot/compose/__init__.py:10`, `bot/compose/repair.py:10`, `bot/knowledge/__init__.py:11`, `bot/knowledge/lookup.py:11`

## `bot/knowledge/work.py`

Imports: `bot/__init__.py:12`, `bot/knowledge/__init__.py:13`, `bot/knowledge/lookup.py:13`, `bot/knowledge/wording.py:14`, `bot/understanding/__init__.py:12`, `bot/understanding/lexicon.py:12`

## `bot/memory/__init__.py`

Imports: `bot/__init__.py:64`, `bot/memory/__init__.py:66`, `bot/memory/facts.py:66`, `bot/memory/filler.py:67`, `bot/memory/resize.py:66`, `bot/memory/topics.py:66`, `bot/understanding/__init__.py:64`, `bot/understanding/kinds.py:64`, `bot/understanding/normalize.py:65`, `bot/understanding/topics.py:1065`

## `bot/memory/a_year_of_birth.py`

Imports: `bot/__init__.py:52`, `bot/compose/__init__.py:90`, `bot/memory/__init__.py:52`

## `bot/memory/aboutthem.py`

Imports: `bot/__init__.py:17`, `bot/memory/__init__.py:17`, `bot/memory/facts.py:18`, `bot/memory/topics.py:17`

## `bot/memory/ack.py`

Imports: `bot/__init__.py:31`, `bot/compose/__init__.py:31`, `bot/compose/badnews.py:32`, `bot/compose/fallback.py:2973`, `bot/compose/fallback_data.py:2982`, `bot/compose/goodnews.py:33`, `bot/compose/loss.py:31`, `bot/compose/trouble.py:31`, `bot/knowledge/__init__.py:4301`, `bot/knowledge/lookup.py:4301`, `bot/memory/__init__.py:40`, `bot/memory/facts.py:43`, `bot/memory/filler.py:2163`, `bot/memory/hours.py:43`, `bot/memory/named.py:43`, `bot/memory/narrow.py:43`, `bot/memory/not_a_trip.py:43`, `bot/memory/opening.py:41`, `bot/memory/relations.py:46`, `bot/memory/store_lines.py:47`, `bot/memory/stuck.py:44`, `bot/memory/taste.py:43`, `bot/memory/topics.py:43`, `bot/memory/tried.py:3878`, `bot/memory/when.py:3991`, `bot/understanding/__init__.py:34`, `bot/understanding/kinds.py:35`, `bot/understanding/normalize.py:37`, `bot/understanding/topics.py:36`

## `bot/memory/again.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:411`, `bot/compose/fallback.py:1230`, `bot/compose/why.py:411`, `bot/memory/__init__.py:23`, `bot/memory/quotes.py:24`, `bot/memory/repeat.py:1286`, `bot/memory/returned.py:1209`, `bot/memory/topics.py:23`, `bot/understanding/__init__.py:22`, `bot/understanding/lexicon.py:22`, `bot/understanding/topics.py:721`

## `bot/memory/already.py`

Imports: `bot/__init__.py:35`, `bot/compose/__init__.py:35`, `bot/compose/fallback.py:867`, `bot/compose/verdict.py:678`, `bot/memory/__init__.py:36`, `bot/memory/limit.py:791`, `bot/memory/plan.py:880`, `bot/memory/repeat.py:36`, `bot/memory/tried.py:1012`, `bot/memory/warmth.py:37`

## `bot/memory/amounts.py`

Imports: none

## `bot/memory/another.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:28`, `bot/compose/whole.py:29`, `bot/memory/__init__.py:30`, `bot/memory/named.py:31`, `bot/memory/repeat.py:31`, `bot/memory/topics.py:30`

## `bot/memory/asked_about.py`

Imports: `bot/__init__.py:30`, `bot/memory/__init__.py:30`, `bot/memory/topics.py:30`

## `bot/memory/asleep.py`

Imports: `bot/__init__.py:35`, `bot/compose/__init__.py:35`

## `bot/memory/back_again.py`

Imports: `bot/__init__.py:29`, `bot/memory/__init__.py:29`, `bot/memory/facts.py:29`, `bot/memory/history.py:29`, `bot/memory/topics.py:29`

## `bot/memory/baking.py`

Imports: none

## `bot/memory/callback.py`

Imports: `bot/__init__.py:18`, `bot/memory/__init__.py:18`, `bot/memory/facts.py:18`, `bot/memory/holds.py:18`

## `bot/memory/carried_on.py`

Imports: `bot/__init__.py:38`, `bot/memory/__init__.py:38`, `bot/memory/tried.py:38`

## `bot/memory/carry.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:570`, `bot/compose/fallback.py:570`, `bot/memory/__init__.py:23`, `bot/memory/again.py:329`, `bot/memory/repair.py:23`, `bot/memory/topics.py:24`, `bot/modules/__init__.py:138`, `bot/modules/answers.py:138`, `bot/understanding/__init__.py:127`

## `bot/memory/cause.py`

Imports: `bot/__init__.py:41`, `bot/compose/__init__.py:41`, `bot/compose/fallback.py:379`, `bot/memory/__init__.py:43`, `bot/memory/another.py:502`, `bot/memory/meant.py:669`, `bot/memory/topics.py:43`, `bot/modules/__init__.py:455`, `bot/modules/chat.py:455`, `bot/understanding/__init__.py:42`

## `bot/memory/change.py`

Imports: none

## `bot/memory/choice.py`

Imports: `bot/__init__.py:21`, `bot/compose/__init__.py:21`, `bot/memory/__init__.py:23`

## `bot/memory/company.py`

Imports: `bot/__init__.py:13`, `bot/compose/__init__.py:13`

## `bot/memory/condition.py`

Imports: `bot/__init__.py:19`, `bot/memory/__init__.py:19`, `bot/memory/facts.py:19`

## `bot/memory/corrected.py`

Imports: `bot/__init__.py:7`, `bot/memory/__init__.py:7`, `bot/memory/facts.py:7`, `bot/memory/named.py:11`, `bot/memory/plans.py:10`, `bot/memory/relations.py:12`, `bot/memory/store_lines.py:9`

## `bot/memory/counts.py`

Imports: `bot/__init__.py:15`, `bot/memory/__init__.py:15`, `bot/memory/facts.py:15`

## `bot/memory/deadline.py`

Imports: `bot/__init__.py:26`, `bot/memory/__init__.py:26`, `bot/memory/topics.py:26`

## `bot/memory/decided.py`

Imports: `bot/__init__.py:14`, `bot/compose/__init__.py:14`

## `bot/memory/detail.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:486`, `bot/compose/trouble.py:486`, `bot/compose/verdict.py:1467`, `bot/knowledge/__init__.py:1212`, `bot/knowledge/lookup.py:1212`, `bot/memory/__init__.py:29`, `bot/memory/again.py:29`, `bot/memory/condition.py:29`, `bot/memory/repeat.py:29`, `bot/memory/topics.py:1151`, `bot/modules/__init__.py:1213`, `bot/modules/answers.py:1213`, `bot/understanding/__init__.py:28`, `bot/understanding/kinds.py:28`

## `bot/memory/doing_it.py`

Imports: `bot/__init__.py:21`, `bot/memory/__init__.py:21`, `bot/memory/topics.py:21`

## `bot/memory/doubt.py`

Imports: `bot/__init__.py:35`, `bot/compose/__init__.py:35`, `bot/memory/__init__.py:36`, `bot/memory/meant.py:36`

## `bot/memory/dream.py`

Imports: none

## `bot/memory/earlier.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:28`

## `bot/memory/enough.py`

Imports: `bot/__init__.py:13`, `bot/knowledge/__init__.py:13`, `bot/knowledge/units.py:13`, `bot/memory/__init__.py:14`, `bot/memory/purse.py:594`, `bot/memory/spoken.py:14`, `bot/memory/store_lines.py:15`

## `bot/memory/facts.py`

Imports: `bot/__init__.py:11`, `bot/compose/__init__.py:2633`, `bot/compose/badnews.py:2633`, `bot/knowledge/__init__.py:1490`, `bot/knowledge/cars.py:5130`, `bot/knowledge/costs.py:1490`, `bot/knowledge/travel.py:1490`, `bot/memory/__init__.py:15`, `bot/memory/filler.py:17`, `bot/memory/named.py:15`, `bot/memory/taste.py:16`, `bot/understanding/__init__.py:11`, `bot/understanding/breeds.py:11`, `bot/understanding/kinds.py:12`, `bot/understanding/lexicon.py:11`, `bot/understanding/normalize.py:11`, `bot/understanding/topics.py:13`

## `bot/memory/favourite.py`

Imports: none

## `bot/memory/filler.py`

Imports: none

## `bot/memory/goals.py`

Imports: none

## `bot/memory/groupages.py`

Imports: none

## `bot/memory/guest.py`

Imports: `bot/__init__.py:111`, `bot/knowledge/__init__.py:117`, `bot/knowledge/cooking.py:117`, `bot/understanding/__init__.py:111`, `bot/understanding/topics.py:111`

## `bot/memory/healing.py`

Imports: none

## `bot/memory/history.py`

Imports: `bot/__init__.py:17`, `bot/compose/__init__.py:645`, `bot/compose/fallback.py:645`, `bot/compose/verdict.py:645`, `bot/memory/__init__.py:17`, `bot/memory/topics.py:17`

## `bot/memory/holds.py`

Imports: none

## `bot/memory/hope.py`

Imports: none

## `bot/memory/hours.py`

Imports: `bot/__init__.py:31`, `bot/compose/__init__.py:31`

## `bot/memory/household.py`

Imports: `bot/__init__.py:30`, `bot/compose/__init__.py:30`, `bot/memory/__init__.py:31`, `bot/memory/counts.py:31`

## `bot/memory/kidcount.py`

Imports: none

## `bot/memory/kinds.py`

Imports: none

## `bot/memory/limit.py`

Imports: `bot/__init__.py:19`, `bot/compose/__init__.py:20`, `bot/knowledge/__init__.py:723`, `bot/knowledge/cooking.py:723`, `bot/memory/__init__.py:21`, `bot/memory/facts.py:347`, `bot/memory/topics.py:21`, `bot/memory/tried.py:21`, `bot/understanding/__init__.py:19`

## `bot/memory/limits.py`

Imports: `bot/__init__.py:36`, `bot/compose/__init__.py:36`

## `bot/memory/lineback.py`

Imports: `bot/__init__.py:30`, `bot/memory/__init__.py:30`, `bot/memory/kinds.py:30`

## `bot/memory/matters.py`

Imports: `bot/__init__.py:30`, `bot/memory/__init__.py:30`, `bot/memory/people.py:30`

## `bot/memory/meant.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/memory/__init__.py:24`, `bot/memory/again.py:24`, `bot/memory/quotes.py:26`, `bot/memory/repeat.py:24`, `bot/memory/topics.py:25`

## `bot/memory/measure.py`

Imports: none

## `bot/memory/more.py`

Imports: `bot/__init__.py:29`, `bot/compose/__init__.py:29`, `bot/memory/__init__.py:30`, `bot/memory/repeat.py:30`

## `bot/memory/named.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:25`, `bot/compose/badnews.py:26`, `bot/compose/draft.py:1124`, `bot/compose/fallback.py:1153`, `bot/compose/trouble.py:25`, `bot/knowledge/__init__.py:1536`, `bot/knowledge/lookup.py:783`, `bot/memory/__init__.py:23`, `bot/memory/facts.py:1171`, `bot/memory/holds.py:23`, `bot/memory/topics.py:24`, `bot/understanding/__init__.py:27`, `bot/understanding/kinds.py:27`, `bot/understanding/normalize.py:435`

## `bot/memory/narrow.py`

Imports: none

## `bot/memory/newborn.py`

Imports: none

## `bot/memory/not_a_trip.py`

Imports: none

## `bot/memory/not_this.py`

Imports: `bot/__init__.py:25`, `bot/compose/__init__.py:25`, `bot/knowledge/__init__.py:26`, `bot/knowledge/travel.py:26`, `bot/memory/__init__.py:27`, `bot/memory/facts.py:27`, `bot/memory/tried.py:249`

## `bot/memory/obstacle.py`

Imports: `bot/__init__.py:49`, `bot/memory/__init__.py:50`, `bot/memory/ack.py:50`, `bot/memory/detail.py:50`, `bot/memory/repeat.py:50`, `bot/memory/topics.py:50`, `bot/understanding/__init__.py:49`

## `bot/memory/onset.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:28`

## `bot/memory/opening.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`

## `bot/memory/outfit.py`

Imports: none

## `bot/memory/outgoings.py`

Imports: none

## `bot/memory/paragraph.py`

Imports: `bot/__init__.py:31`, `bot/memory/__init__.py:31`, `bot/memory/again.py:31`

## `bot/memory/passing_the_time.py`

Imports: `bot/__init__.py:34`, `bot/compose/__init__.py:34`

## `bot/memory/pending.py`

Imports: `bot/__init__.py:25`, `bot/knowledge/__init__.py:25`, `bot/knowledge/lookup.py:25`, `bot/memory/__init__.py:27`, `bot/memory/plans.py:27`, `bot/understanding/__init__.py:26`

## `bot/memory/people.py`

Imports: `bot/__init__.py:87`, `bot/memory/__init__.py:87`, `bot/memory/topics.py:88`

## `bot/memory/pile.py`

Imports: `bot/__init__.py:32`, `bot/compose/__init__.py:32`

## `bot/memory/places.py`

Imports: `bot/__init__.py:14`, `bot/memory/__init__.py:14`, `bot/memory/facts.py:14`, `bot/memory/store_lines.py:16`

## `bot/memory/plan.py`

Imports: `bot/__init__.py:34`, `bot/compose/__init__.py:34`, `bot/memory/__init__.py:35`, `bot/memory/repeat.py:35`

## `bot/memory/plans.py`

Imports: `bot/__init__.py:25`, `bot/compose/__init__.py:25`, `bot/knowledge/__init__.py:26`, `bot/knowledge/lookup.py:26`, `bot/memory/__init__.py:28`, `bot/memory/when.py:28`, `bot/understanding/__init__.py:27`

## `bot/memory/portions.py`

Imports: `bot/__init__.py:40`, `bot/memory/__init__.py:40`, `bot/memory/history.py:40`

## `bot/memory/prefer.py`

Imports: none

## `bot/memory/price.py`

Imports: `bot/__init__.py:21`, `bot/compose/__init__.py:21`, `bot/compose/prices.py:525`

## `bot/memory/purse.py`

Imports: none

## `bot/memory/quotes.py`

Imports: none

## `bot/memory/rates.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`

## `bot/memory/recall.py`

Imports: `bot/__init__.py:21`, `bot/memory/__init__.py:22`, `bot/memory/topics.py:22`, `bot/understanding/__init__.py:21`, `bot/understanding/breeds.py:21`

## `bot/memory/reject.py`

Imports: `bot/__init__.py:27`, `bot/compose/__init__.py:27`, `bot/memory/__init__.py:28`, `bot/memory/choice.py:28`

## `bot/memory/relations.py`

Imports: `bot/__init__.py:10`, `bot/compose/__init__.py:10`, `bot/memory/__init__.py:11`, `bot/memory/facts.py:12`, `bot/memory/history.py:11`, `bot/memory/store_lines.py:17`, `bot/understanding/__init__.py:29`, `bot/understanding/breeds.py:29`, `bot/understanding/normalize.py:30`

## `bot/memory/remedy.py`

Imports: none

## `bot/memory/reminded.py`

Imports: `bot/__init__.py:25`, `bot/memory/__init__.py:25`, `bot/memory/topics.py:26`

## `bot/memory/rename.py`

Imports: `bot/__init__.py:29`, `bot/memory/__init__.py:29`, `bot/memory/facts.py:29`

## `bot/memory/repair.py`

Imports: `bot/__init__.py:46`, `bot/compose/__init__.py:46`, `bot/memory/__init__.py:47`, `bot/memory/facts.py:48`, `bot/memory/topics.py:49`, `bot/modules/__init__.py:463`, `bot/modules/answers.py:463`, `bot/understanding/__init__.py:251`

## `bot/memory/repeat.py`

Imports: `bot/__init__.py:24`, `bot/compose/__init__.py:24`, `bot/compose/fallback.py:2853`, `bot/compose/limit.py:2440`, `bot/compose/verdict.py:1110`, `bot/compose/whole.py:25`, `bot/knowledge/__init__.py:2403`, `bot/knowledge/plans.py:2403`, `bot/knowledge/travel.py:2431`, `bot/memory/__init__.py:27`, `bot/memory/counts.py:27`, `bot/memory/facts.py:27`, `bot/memory/history.py:27`, `bot/memory/limits.py:27`, `bot/memory/narrow.py:27`, `bot/memory/recall.py:27`, `bot/memory/stuck.py:4286`, `bot/memory/topics.py:27`, `bot/modules/__init__.py:2641`, `bot/modules/answers.py:2641`, `bot/understanding/__init__.py:26`

## `bot/memory/resize.py`

Imports: `bot/__init__.py:128`, `bot/memory/__init__.py:128`

## `bot/memory/returned.py`

Imports: none

## `bot/memory/returns.py`

Imports: `bot/__init__.py:26`, `bot/memory/__init__.py:26`, `bot/memory/again.py:241`, `bot/memory/topics.py:26`

## `bot/memory/saving.py`

Imports: none

## `bot/memory/setback.py`

Imports: none

## `bot/memory/size.py`

Imports: none

## `bot/memory/spent.py`

Imports: none

## `bot/memory/spoken.py`

Imports: none

## `bot/memory/stands.py`

Imports: `bot/__init__.py:36`, `bot/compose/__init__.py:36`

## `bot/memory/store_lines.py`

Imports: none

## `bot/memory/store_replies.py`

Imports: `bot/__init__.py:10`, `bot/compose/__init__.py:10`, `bot/compose/fallback.py:11575`, `bot/compose/own_facts.py:4683`, `bot/compose/recall.py:12`, `bot/compose/ruledout.py:11`, `bot/knowledge/__init__.py:38`, `bot/knowledge/cooking.py:38`, `bot/knowledge/maths.py:40`, `bot/knowledge/units.py:39`, `bot/memory/__init__.py:13`, `bot/memory/ack.py:17`, `bot/memory/again.py:4365`, `bot/memory/another.py:25`, `bot/memory/carry.py:49`, `bot/memory/cause.py:23`, `bot/memory/change.py:21`, `bot/memory/choice.py:26`, `bot/memory/company.py:29`, `bot/memory/condition.py:27`, `bot/memory/corrected.py:278`, `bot/memory/counts.py:41`, `bot/memory/decided.py:28`, `bot/memory/enough.py:208`, `bot/memory/facts.py:37`, `bot/memory/filler.py:32`, `bot/memory/guest.py:33`, `bot/memory/history.py:44`, `bot/memory/named.py:46`, `bot/memory/narrow.py:45`, `bot/memory/pending.py:24`, `bot/memory/places.py:18`, `bot/memory/plans.py:34`, `bot/memory/rates.py:42`, `bot/memory/recall.py:47`, `bot/memory/reject.py:51`, `bot/memory/relations.py:209`, `bot/memory/reminded.py:48`, `bot/memory/repair.py:52`, `bot/memory/repeat.py:59`, `bot/memory/resize.py:50`, `bot/memory/spent.py:30`, `bot/memory/store_lines.py:67`, `bot/memory/surname.py:31`, `bot/memory/taste.py:35`, `bot/memory/thread.py:13`, `bot/memory/times.py:43`, `bot/memory/told.py:53`, `bot/memory/topics.py:57`, `bot/memory/whathurts.py:54`, `bot/memory/when.py:22`, `bot/modules/__init__.py:235`, `bot/modules/answers.py:4364`, `bot/modules/memory_tables.py:235`, `bot/understanding/__init__.py:63`, `bot/understanding/breeds.py:63`, `bot/understanding/kinds.py:65`, `bot/understanding/normalize.py:64`, `bot/understanding/topics.py:66`

## `bot/memory/stuck.py`

Imports: `bot/__init__.py:20`, `bot/compose/__init__.py:20`, `bot/compose/badnews.py:20`, `bot/memory/__init__.py:21`, `bot/memory/topics.py:21`

## `bot/memory/surname.py`

Imports: `bot/__init__.py:189`, `bot/memory/__init__.py:189`

## `bot/memory/swap.py`

Imports: `bot/__init__.py:237`, `bot/memory/__init__.py:237`, `bot/memory/topics.py:237`, `bot/understanding/__init__.py:301`

## `bot/memory/taking.py`

Imports: `bot/__init__.py:29`, `bot/knowledge/__init__.py:132`, `bot/knowledge/body.py:132`, `bot/memory/__init__.py:29`, `bot/memory/topics.py:29`, `bot/modules/__init__.py:231`, `bot/modules/answers.py:231`

## `bot/memory/taste.py`

Imports: none

## `bot/memory/the_verdict.py`

Imports: none

## `bot/memory/the_years.py`

Imports: `bot/__init__.py:34`, `bot/compose/__init__.py:34`, `bot/memory/__init__.py:35`, `bot/memory/passing_the_time.py:35`

## `bot/memory/their_feeling.py`

Imports: `bot/__init__.py:45`, `bot/compose/__init__.py:45`, `bot/compose/remark.py:45`, `bot/memory/__init__.py:46`, `bot/memory/history.py:178`, `bot/memory/relations.py:46`

## `bot/memory/thread.py`

Imports: `bot/__init__.py:20`, `bot/compose/__init__.py:24`, `bot/memory/__init__.py:25`, `bot/memory/detail.py:25`, `bot/memory/facts.py:27`, `bot/memory/topics.py:673`, `bot/understanding/__init__.py:20`, `bot/understanding/kinds.py:21`, `bot/understanding/normalize.py:22`, `bot/understanding/topics.py:23`

## `bot/memory/times.py`

Imports: `bot/__init__.py:10`, `bot/memory/__init__.py:10`, `bot/memory/store_lines.py:11`

## `bot/memory/told.py`

Imports: `bot/__init__.py:25`, `bot/compose/__init__.py:25`, `bot/memory/__init__.py:27`, `bot/memory/facts.py:29`, `bot/memory/history.py:27`, `bot/memory/recall.py:30`, `bot/memory/store_replies.py:784`, `bot/memory/when.py:760`, `bot/understanding/__init__.py:26`, `bot/understanding/breeds.py:26`

## `bot/memory/too_late.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/memory/__init__.py:24`, `bot/memory/meant.py:24`

## `bot/memory/topics.py`

Imports: `bot/__init__.py:20`, `bot/compose/__init__.py:8469`, `bot/compose/fallback.py:8469`, `bot/knowledge/__init__.py:2713`, `bot/knowledge/body.py:2713`, `bot/knowledge/lookup.py:2844`, `bot/knowledge/travel.py:3100`, `bot/memory/__init__.py:22`, `bot/memory/again.py:4530`, `bot/memory/already.py:3989`, `bot/memory/detail.py:3435`, `bot/memory/facts.py:299`, `bot/memory/filler.py:22`, `bot/memory/named.py:7960`, `bot/memory/narrow.py:4925`, `bot/memory/quotes.py:23`, `bot/memory/repeat.py:3989`, `bot/memory/returned.py:4537`, `bot/memory/when.py:7915`, `bot/understanding/__init__.py:20`, `bot/understanding/breeds.py:7804`, `bot/understanding/kinds.py:21`, `bot/understanding/lexicon.py:20`

## `bot/memory/tried.py`

Imports: `bot/__init__.py:19`, `bot/compose/__init__.py:1035`, `bot/compose/verdict.py:1035`, `bot/memory/__init__.py:19`, `bot/memory/facts.py:22`, `bot/memory/named.py:1559`, `bot/memory/repeat.py:1070`, `bot/memory/topics.py:1245`

## `bot/memory/trips.py`

Imports: `bot/__init__.py:27`, `bot/knowledge/__init__.py:833`, `bot/knowledge/travel.py:833`, `bot/memory/__init__.py:27`, `bot/memory/ack.py:27`, `bot/memory/facts.py:27`, `bot/memory/not_a_trip.py:27`

## `bot/memory/warmth.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:28`, `bot/memory/__init__.py:29`, `bot/memory/history.py:29`

## `bot/memory/whathurts.py`

Imports: `bot/__init__.py:12`, `bot/memory/__init__.py:12`, `bot/memory/facts.py:12`, `bot/memory/told.py:14`

## `bot/memory/when.py`

Imports: `bot/__init__.py:20`, `bot/memory/__init__.py:20`, `bot/memory/facts.py:20`, `bot/memory/topics.py:22`

## `bot/memory/whereabout.py`

Imports: none

## `bot/memory/whose_body.py`

Imports: none

## `bot/memory/will_it_work.py`

Imports: `bot/__init__.py:30`, `bot/memory/__init__.py:30`, `bot/memory/measure.py:30`

## `bot/memory/workplace.py`

Imports: `bot/__init__.py:27`, `bot/compose/__init__.py:27`

## `bot/memory/years_in.py`

Imports: `bot/__init__.py:27`, `bot/compose/__init__.py:27`, `bot/memory/__init__.py:28`, `bot/memory/facts.py:28`

## `bot/modules/__init__.py`

Imports: none

## `bot/modules/a_cake_between_its_layers.py`

Imports: `bot/__init__.py:26`, `bot/knowledge/__init__.py:26`, `bot/knowledge/lookup.py:26`, `bot/understanding/__init__.py:27`

## `bot/modules/a_check_with_you.py`

Imports: `bot/__init__.py:12`, `bot/compose/__init__.py:12`, `bot/compose/check_with_you.py:12`

## `bot/modules/a_count_of_packets.py`

Imports: `bot/__init__.py:14`, `bot/compose/__init__.py:14`, `bot/compose/a_count_of_packets.py:15`, `bot/knowledge/__init__.py:16`, `bot/knowledge/crisis_replies.py:16`, `bot/modules/__init__.py:17`, `bot/modules/safety.py:17`, `bot/modules/urgent.py:17`

## `bot/modules/a_count_said.py`

Imports: `bot/__init__.py:18`, `bot/memory/__init__.py:18`, `bot/memory/amounts.py:18`

## `bot/modules/a_day_somebody_else_keeps.py`

Imports: `bot/__init__.py:29`, `bot/memory/__init__.py:29`

## `bot/modules/a_debt_said_flat.py`

Imports: `bot/__init__.py:29`, `bot/compose/__init__.py:29`

## `bot/modules/a_fact_asked_again.py`

Imports: `bot/__init__.py:39`, `bot/memory/__init__.py:39`, `bot/memory/when.py:39`

## `bot/modules/a_figure_beside_the_state.py`

Imports: `bot/__init__.py:102`, `bot/compose/__init__.py:102`, `bot/compose/fallback.py:102`, `bot/compose/fallback_chain.py:103`, `bot/compose/fallback_data.py:119`

## `bot/modules/a_figure_of_a_dish.py`

Imports: none

## `bot/modules/a_first_time.py`

Imports: `bot/__init__.py:8`, `bot/compose/__init__.py:8`, `bot/compose/a_first_time.py:8`

## `bot/modules/a_habit_is_raised.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`, `bot/compose/fallback.py:22`, `bot/compose/habit.py:22`, `bot/memory/__init__.py:23`, `bot/memory/ack.py:23`

## `bot/modules/a_job_that_is_not_a_diy_job.py`

Imports: `bot/__init__.py:27`, `bot/memory/__init__.py:27`

## `bot/modules/a_letter_about_money.py`

Imports: none

## `bot/modules/a_limit_over_the_answer.py`

Imports: none

## `bot/modules/a_measure_asked_again.py`

Imports: `bot/__init__.py:13`, `bot/memory/__init__.py:13`, `bot/memory/measure.py:13`

## `bot/modules/a_number_the_bot_knows.py`

Imports: `bot/__init__.py:24`, `bot/compose/__init__.py:24`, `bot/compose/a_number_the_bot_knows.py:24`

## `bot/modules/a_plan_the_thread_holds.py`

Imports: `bot/__init__.py:20`, `bot/compose/__init__.py:20`, `bot/compose/fallback.py:20`, `bot/compose/plan_ask.py:20`

## `bot/modules/a_question_about_somebody.py`

Imports: `bot/__init__.py:33`, `bot/compose/__init__.py:33`

## `bot/modules/a_return_brings_back_the_advice.py`

Imports: `bot/__init__.py:24`, `bot/memory/__init__.py:24`, `bot/memory/topics.py:24`

## `bot/modules/a_shock_at_a_bill.py`

Imports: `bot/__init__.py:137`, `bot/compose/__init__.py:137`, `bot/compose/fallback_data.py:137`, `bot/modules/__init__.py:392`, `bot/modules/answers.py:392`

## `bot/modules/a_thing_being_weighed.py`

Imports: `bot/__init__.py:32`, `bot/compose/__init__.py:32`, `bot/compose/a_thing_being_weighed.py:32`, `bot/modules/__init__.py:35`, `bot/modules/greeting.py:35`

## `bot/modules/a_thing_wants_doing.py`

Imports: `bot/__init__.py:21`, `bot/compose/__init__.py:21`, `bot/compose/a_thing_wants_doing.py:21`, `bot/compose/fallback.py:21`, `bot/modules/__init__.py:22`, `bot/modules/greeting.py:22`

## `bot/modules/a_will_it_work.py`

Imports: `bot/__init__.py:14`, `bot/memory/__init__.py:14`, `bot/memory/will_it_work.py:14`

## `bot/modules/aboutachild.py`

Imports: `bot/__init__.py:13`, `bot/memory/__init__.py:13`, `bot/memory/aboutthem.py:13`, `bot/modules/__init__.py:15`, `bot/modules/answers.py:15`, `bot/understanding/__init__.py:14`

## `bot/modules/after_a_lead_in.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`, `bot/compose/lead_in.py:22`

## `bot/modules/already.py`

Imports: `bot/__init__.py:10`, `bot/memory/__init__.py:10`, `bot/memory/already.py:10`

## `bot/modules/an_age_ahead.py`

Imports: `bot/__init__.py:16`, `bot/memory/__init__.py:16`, `bot/memory/aboutthem.py:16`

## `bot/modules/answers.py`

Imports: `bot/__init__.py:15`, `bot/compose/__init__.py:16`, `bot/compose/fallback.py:6771`, `bot/compose/fit.py:17`, `bot/compose/goodnews.py:16`, `bot/compose/lead.py:16`, `bot/compose/reading.py:8272`, `bot/compose/repair.py:18`, `bot/compose/said_about_me.py:8348`, `bot/compose/trouble.py:19`, `bot/knowledge/__init__.py:20`, `bot/knowledge/cooking.py:20`, `bot/knowledge/dishes.py:20`, `bot/knowledge/entertainment.py:20`, `bot/knowledge/freshness.py:20`, `bot/knowledge/home.py:20`, `bot/knowledge/lookup.py:22`, `bot/knowledge/maths.py:34`, `bot/knowledge/meat_in_a_card.py:20`, `bot/knowledge/pets.py:20`, `bot/knowledge/shopping.py:20`, `bot/knowledge/travel.py:8834`, `bot/knowledge/work.py:20`, `bot/memory/__init__.py:15`, `bot/memory/a_year_of_birth.py:36`, `bot/memory/ack.py:29`, `bot/memory/again.py:43`, `bot/memory/baking.py:37`, `bot/memory/deadline.py:29`, `bot/memory/detail.py:8047`, `bot/memory/doing_it.py:37`, `bot/memory/healing.py:29`, `bot/memory/hope.py:32`, `bot/memory/limit.py:61`, `bot/memory/matters.py:32`, `bot/memory/meant.py:8294`, `bot/memory/more.py:59`, `bot/memory/named.py:29`, `bot/memory/narrow.py:29`, `bot/memory/not_this.py:8343`, `bot/memory/onset.py:29`, `bot/memory/outfit.py:39`, `bot/memory/paragraph.py:12263`, `bot/memory/portions.py:8344`, `bot/memory/prefer.py:29`, `bot/memory/price.py:63`, `bot/memory/recall.py:40`, `bot/memory/repeat.py:29`, `bot/memory/resize.py:40`, `bot/memory/returned.py:59`, `bot/memory/returns.py:60`, `bot/memory/size.py:8345`, `bot/memory/stuck.py:42`, `bot/memory/swap.py:64`, `bot/memory/taste.py:29`, `bot/memory/thread.py:8347`, `bot/memory/topics.py:33`, `bot/memory/tried.py:59`, `bot/memory/trips.py:37`, `bot/memory/when.py:29`, `bot/memory/whereabout.py:62`, `bot/understanding/__init__.py:35`, `bot/understanding/kinds.py:35`, `bot/understanding/normalize.py:10451`, `bot/understanding/topics.py:6547`

## `bot/modules/arbiter.py`

Imports: `bot/__init__.py:66`, `bot/compose/__init__.py:66`, `bot/compose/aboutness.py:66`, `bot/compose/badnews.py:66`, `bot/compose/childwords.py:4424`, `bot/compose/facts.py:66`, `bot/compose/fallback.py:66`, `bot/compose/fit.py:66`, `bot/compose/goodnews.py:70`, `bot/compose/harm.py:93`, `bot/compose/judgement.py:2389`, `bot/compose/leaving.py:66`, `bot/compose/limit.py:68`, `bot/compose/loss.py:81`, `bot/compose/onscreen.py:66`, `bot/compose/outlook.py:67`, `bot/compose/reading.py:138`, `bot/compose/ruledout.py:129`, `bot/compose/verdict.py:1950`, `bot/knowledge/__init__.py:125`, `bot/knowledge/cooking.py:125`, `bot/memory/__init__.py:69`, `bot/memory/ack.py:72`, `bot/memory/already.py:89`, `bot/memory/dream.py:85`, `bot/memory/history.py:134`, `bot/memory/lineback.py:109`, `bot/memory/meant.py:2186`, `bot/memory/named.py:113`, `bot/memory/narrow.py:69`, `bot/memory/recall.py:76`, `bot/memory/repeat.py:77`, `bot/memory/returned.py:3217`, `bot/memory/topics.py:3243`, `bot/modules/__init__.py:2162`, `bot/modules/answers.py:3249`, `bot/modules/memory_store.py:2162`, `bot/understanding/__init__.py:5202`, `bot/understanding/normalize.py:5203`, `bot/understanding/topics.py:5202`

## `bot/modules/attribute_said_again.py`

Imports: `bot/__init__.py:61`, `bot/memory/__init__.py:61`, `bot/memory/repeat.py:61`, `bot/modules/__init__.py:215`, `bot/modules/memory_store.py:215`

## `bot/modules/bulky.py`

Imports: none

## `bot/modules/callback_about.py`

Imports: `bot/__init__.py:12`, `bot/memory/__init__.py:12`, `bot/memory/callback.py:12`

## `bot/modules/carers_turn.py`

Imports: `bot/__init__.py:43`, `bot/compose/__init__.py:43`, `bot/compose/fallback.py:43`, `bot/compose/fallback_data.py:44`

## `bot/modules/chasing_a_body.py`

Imports: `bot/__init__.py:197`, `bot/compose/__init__.py:197`, `bot/compose/fallback.py:197`

## `bot/modules/chat.py`

Imports: `bot/__init__.py:11`, `bot/compose/__init__.py:11`, `bot/compose/badnews.py:11`, `bot/compose/circumstance.py:11`, `bot/compose/doubt.py:45`, `bot/compose/fallback.py:11`, `bot/compose/goodnews.py:16`, `bot/compose/harm.py:34`, `bot/compose/leaving.py:13`, `bot/compose/limit.py:14`, `bot/compose/loss.py:31`, `bot/compose/occasionplan.py:46`, `bot/compose/onscreen.py:32`, `bot/compose/reaction.py:36`, `bot/compose/reading.py:41`, `bot/compose/repair.py:37`, `bot/compose/said_about_me.py:38`, `bot/compose/saidby.py:35`, `bot/compose/stepwords.py:43`, `bot/compose/trouble.py:14889`, `bot/compose/unseen.py:33`, `bot/compose/urgent.py:42`, `bot/compose/verdict.py:44`, `bot/knowledge/__init__.py:26`, `bot/knowledge/cooking.py:4891`, `bot/knowledge/lookup.py:26`, `bot/knowledge/travel.py:69`, `bot/memory/__init__.py:22`, `bot/memory/ack.py:50`, `bot/memory/again.py:59`, `bot/memory/carried_on.py:60`, `bot/memory/cause.py:25`, `bot/memory/facts.py:54`, `bot/memory/goals.py:11866`, `bot/memory/hope.py:22`, `bot/memory/hours.py:65`, `bot/memory/limit.py:65`, `bot/memory/limits.py:64`, `bot/memory/matters.py:22`, `bot/memory/meant.py:65`, `bot/memory/more.py:65`, `bot/memory/narrow.py:14346`, `bot/memory/obstacle.py:22`, `bot/memory/passing_the_time.py:61`, `bot/memory/pile.py:65`, `bot/memory/plan.py:22`, `bot/memory/portions.py:64`, `bot/memory/remedy.py:51`, `bot/memory/repeat.py:65`, `bot/memory/returned.py:65`, `bot/memory/size.py:24`, `bot/memory/stuck.py:52`, `bot/memory/the_years.py:62`, `bot/memory/thread.py:53`, `bot/memory/topics.py:55`, `bot/memory/tried.py:65`, `bot/memory/warmth.py:63`, `bot/memory/when.py:23`, `bot/modules/__init__.py:3604`, `bot/modules/answers.py:4800`, `bot/modules/chat_replies.py:3604`, `bot/modules/particles.py:12679`, `bot/understanding/__init__.py:27`, `bot/understanding/breeds.py:27`, `bot/understanding/kinds.py:28`, `bot/understanding/lexicon.py:70`, `bot/understanding/topics.py:27`

## `bot/modules/chat_replies.py`

Imports: `bot/__init__.py:13`, `bot/compose/__init__.py:13`, `bot/compose/fallback.py:13`

## `bot/modules/colour_on_the_body.py`

Imports: `bot/__init__.py:13`, `bot/compose/__init__.py:13`, `bot/compose/colour_on_the_body.py:13`

## `bot/modules/confusing.py`

Imports: `bot/__init__.py:27`, `bot/compose/__init__.py:27`, `bot/compose/fallback.py:27`, `bot/compose/hedge.py:27`

## `bot/modules/detail_question.py`

Imports: `bot/__init__.py:13`, `bot/compose/__init__.py:13`, `bot/compose/their_detail.py:13`

## `bot/modules/done_ask.py`

Imports: `bot/__init__.py:35`, `bot/knowledge/__init__.py:62`, `bot/knowledge/lookup.py:62`, `bot/memory/__init__.py:35`, `bot/memory/topics.py:35`

## `bot/modules/doubt.py`

Imports: `bot/__init__.py:15`, `bot/memory/__init__.py:15`, `bot/memory/doubt.py:15`

## `bot/modules/dreams.py`

Imports: `bot/__init__.py:29`, `bot/memory/__init__.py:29`, `bot/memory/dream.py:29`

## `bot/modules/familyask.py`

Imports: `bot/__init__.py:34`, `bot/compose/__init__.py:34`, `bot/compose/fallback.py:34`

## `bot/modules/favourites.py`

Imports: `bot/__init__.py:19`, `bot/compose/__init__.py:19`, `bot/knowledge/__init__.py:20`, `bot/knowledge/lookup.py:20`, `bot/memory/__init__.py:21`, `bot/memory/favourite.py:21`, `bot/memory/taste.py:22`

## `bot/modules/figure_asked_again.py`

Imports: `bot/__init__.py:19`, `bot/memory/__init__.py:19`, `bot/memory/stands.py:19`

## `bot/modules/first_line_open.py`

Imports: `bot/__init__.py:16`, `bot/compose/__init__.py:16`, `bot/compose/fallback.py:16`, `bot/compose/fallback_data.py:17`, `bot/modules/__init__.py:18`, `bot/modules/greeting.py:18`

## `bot/modules/greeting.py`

Imports: `bot/__init__.py:4`, `bot/compose/__init__.py:4`, `bot/compose/a_detail_dropped.py:1278`, `bot/compose/a_document_lives_with_an_office.py:30`, `bot/compose/a_place_nobody_can_see.py:31`, `bot/compose/breakup.py:4`, `bot/compose/checking.py:11`, `bot/compose/fallback.py:5`, `bot/compose/find_a_place.py:28`, `bot/compose/findout.py:6`, `bot/compose/gap.py:6`, `bot/compose/goodnews.py:12`, `bot/compose/griefplea.py:13`, `bot/compose/hands.py:14`, `bot/compose/how_it_went.py:15`, `bot/compose/is_it_worth_it.py:417`, `bot/compose/liveask.py:16`, `bot/compose/missing_half.py:17`, `bot/compose/moving.py:7`, `bot/compose/no_eyes.py:37`, `bot/compose/no_page.py:9`, `bot/compose/nottheirbest.py:36`, `bot/compose/only_they_know.py:22`, `bot/compose/onmybehalf.py:29`, `bot/compose/onscreen.py:18`, `bot/compose/own_facts.py:19`, `bot/compose/plans.py:19`, `bot/compose/pleased.py:33`, `bot/compose/quantity.py:8`, `bot/compose/remark.py:39`, `bot/compose/rotten_luck.py:34`, `bot/compose/rule.py:23`, `bot/compose/safety_doubt.py:21`, `bot/compose/said_about_me.py:35`, `bot/compose/setting_off.py:40`, `bot/compose/stretches.py:20`, `bot/compose/taste.py:26`, `bot/compose/the_course_of_it.py:10`, `bot/compose/the_turn_after.py:27`, `bot/compose/their_day.py:24`, `bot/compose/their_life.py:25`, `bot/compose/tone_again.py:232`, `bot/compose/what_i_do_know.py:749`, `bot/compose/who_to_see.py:38`, `bot/memory/__init__.py:32`, `bot/memory/ack.py:32`, `bot/memory/again.py:1266`, `bot/memory/asked_about.py:657`, `bot/memory/back_again.py:42`, `bot/memory/detail.py:32`, `bot/memory/matters.py:32`, `bot/memory/obstacle.py:32`, `bot/memory/repeat.py:32`, `bot/memory/the_verdict.py:375`, `bot/memory/their_feeling.py:43`, `bot/memory/too_late.py:32`, `bot/memory/tried.py:108`

## `bot/modules/grief.py`

Imports: `bot/__init__.py:17`, `bot/compose/__init__.py:17`, `bot/compose/loss.py:17`, `bot/knowledge/__init__.py:19`, `bot/knowledge/lookup.py:19`, `bot/modules/__init__.py:867`, `bot/modules/answers.py:867`

## `bot/modules/group_ages.py`

Imports: `bot/__init__.py:25`, `bot/memory/__init__.py:25`, `bot/memory/groupages.py:25`

## `bot/modules/hardpast.py`

Imports: `bot/__init__.py:59`, `bot/compose/__init__.py:59`

## `bot/modules/household.py`

Imports: `bot/__init__.py:18`, `bot/memory/__init__.py:18`, `bot/memory/household.py:18`

## `bot/modules/how_long_a_stay.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:28`

## `bot/modules/intent.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`, `bot/compose/clause.py:22`, `bot/compose/fit.py:22`, `bot/compose/what_it_asks.py:22`

## `bot/modules/invitation.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`, `bot/compose/invitation.py:22`

## `bot/modules/judge_ask.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/compose/fallback.py:23`, `bot/modules/__init__.py:24`, `bot/modules/greeting.py:24`

## `bot/modules/keeping.py`

Imports: `bot/__init__.py:23`, `bot/understanding/__init__.py:23`, `bot/understanding/kinds.py:23`

## `bot/modules/kid_tally.py`

Imports: `bot/__init__.py:12`, `bot/memory/__init__.py:12`, `bot/memory/kidcount.py:12`

## `bot/modules/live_source.py`

Imports: none

## `bot/modules/live_times.py`

Imports: none

## `bot/modules/loss.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`, `bot/compose/fallback.py:154`, `bot/compose/fit.py:22`, `bot/compose/loss.py:22`, `bot/understanding/__init__.py:23`

## `bot/modules/memory_store.py`

Imports: `bot/__init__.py:18`, `bot/compose/__init__.py:18`, `bot/compose/fallback.py:6015`, `bot/compose/recall.py:19`, `bot/knowledge/__init__.py:46`, `bot/knowledge/cooking.py:46`, `bot/knowledge/maths.py:48`, `bot/knowledge/units.py:47`, `bot/memory/__init__.py:20`, `bot/memory/ack.py:24`, `bot/memory/again.py:4205`, `bot/memory/another.py:32`, `bot/memory/carry.py:60`, `bot/memory/cause.py:29`, `bot/memory/change.py:28`, `bot/memory/choice.py:33`, `bot/memory/company.py:36`, `bot/memory/condition.py:34`, `bot/memory/corrected.py:2317`, `bot/memory/counts.py:49`, `bot/memory/decided.py:35`, `bot/memory/enough.py:224`, `bot/memory/facts.py:45`, `bot/memory/filler.py:39`, `bot/memory/goals.py:4243`, `bot/memory/guest.py:40`, `bot/memory/history.py:54`, `bot/memory/hours.py:56`, `bot/memory/named.py:42`, `bot/memory/narrow.py:55`, `bot/memory/not_this.py:806`, `bot/memory/pending.py:31`, `bot/memory/places.py:25`, `bot/memory/plans.py:41`, `bot/memory/rates.py:50`, `bot/memory/recall.py:58`, `bot/memory/reject.py:62`, `bot/memory/relations.py:229`, `bot/memory/reminded.py:59`, `bot/memory/rename.py:2558`, `bot/memory/repair.py:63`, `bot/memory/repeat.py:70`, `bot/memory/resize.py:61`, `bot/memory/spent.py:37`, `bot/memory/spoken.py:30`, `bot/memory/store_lines.py:80`, `bot/memory/store_replies.py:2411`, `bot/memory/surname.py:38`, `bot/memory/taste.py:43`, `bot/memory/thread.py:20`, `bot/memory/times.py:51`, `bot/memory/told.py:64`, `bot/memory/topics.py:68`, `bot/memory/trips.py:1056`, `bot/memory/whathurts.py:65`, `bot/memory/when.py:66`, `bot/modules/__init__.py:255`, `bot/modules/memory_tables.py:255`, `bot/understanding/__init__.py:74`, `bot/understanding/breeds.py:74`, `bot/understanding/kinds.py:76`, `bot/understanding/normalize.py:75`, `bot/understanding/topics.py:77`

## `bot/modules/memory_tables.py`

Imports: `bot/__init__.py:16`, `bot/memory/__init__.py:16`, `bot/memory/store_lines.py:16`

## `bot/modules/my_work.py`

Imports: `bot/__init__.py:8`, `bot/memory/__init__.py:8`, `bot/memory/workplace.py:8`

## `bot/modules/name_taken_back.py`

Imports: `bot/__init__.py:27`, `bot/compose/__init__.py:27`, `bot/memory/__init__.py:28`, `bot/memory/facts.py:28`

## `bot/modules/near_me.py`

Imports: `bot/__init__.py:13`, `bot/compose/__init__.py:13`, `bot/compose/nearby_one.py:13`

## `bot/modules/night.py`

Imports: `bot/__init__.py:24`, `bot/knowledge/__init__.py:24`, `bot/knowledge/sleep_aids.py:24`, `bot/memory/__init__.py:25`, `bot/memory/asleep.py:25`

## `bot/modules/no_advice.py`

Imports: `bot/__init__.py:25`, `bot/compose/__init__.py:25`, `bot/compose/no_advice.py:25`

## `bot/modules/nudge.py`

Imports: `bot/__init__.py:24`, `bot/compose/__init__.py:24`, `bot/compose/still.py:24`

## `bot/modules/on_record.py`

Imports: `bot/__init__.py:28`, `bot/compose/__init__.py:28`, `bot/compose/fit.py:28`

## `bot/modules/onsubject.py`

Imports: `bot/__init__.py:29`, `bot/compose/__init__.py:29`, `bot/compose/fallback.py:29`, `bot/compose/onsubject.py:30`

## `bot/modules/pace.py`

Imports: `bot/__init__.py:18`, `bot/compose/__init__.py:18`, `bot/compose/pace.py:18`

## `bot/modules/particles.py`

Imports: none

## `bot/modules/recap.py`

Imports: `bot/__init__.py:27`, `bot/memory/__init__.py:95`, `bot/memory/thread.py:95`, `bot/modules/__init__.py:27`, `bot/modules/memory_store.py:27`

## `bot/modules/receiving_end.py`

Imports: `bot/__init__.py:9`, `bot/compose/__init__.py:9`, `bot/compose/receiving_end.py:9`

## `bot/modules/record_due.py`

Imports: none

## `bot/modules/ruled_out.py`

Imports: `bot/__init__.py:19`, `bot/compose/__init__.py:19`, `bot/compose/ruledout.py:19`

## `bot/modules/safety.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`, `bot/compose/fallback.py:23`, `bot/knowledge/__init__.py:5664`, `bot/knowledge/crisis_replies.py:5664`, `bot/memory/__init__.py:12014`, `bot/memory/again.py:12014`, `bot/memory/dream.py:25`, `bot/modules/__init__.py:35`, `bot/modules/safety_cards.py:35`, `bot/modules/safety_phrases.py:2468`, `bot/modules/safety_tables.py:95`, `bot/understanding/__init__.py:28`, `bot/understanding/lexicon.py:29`, `bot/understanding/normalize.py:32`

## `bot/modules/safety_cards.py`

Imports: none

## `bot/modules/safety_phrases.py`

Imports: `bot/__init__.py:10`, `bot/modules/__init__.py:10`, `bot/modules/safety_cards.py:10`

## `bot/modules/safety_tables.py`

Imports: `bot/__init__.py:9`, `bot/modules/__init__.py:9`, `bot/modules/safety_cards.py:9`, `bot/modules/safety_phrases.py:62`

## `bot/modules/said_back.py`

Imports: `bot/__init__.py:13`, `bot/memory/__init__.py:13`, `bot/memory/lineback.py:13`

## `bot/modules/said_fact_back.py`

Imports: `bot/__init__.py:34`, `bot/compose/__init__.py:57`, `bot/compose/fallback.py:57`, `bot/compose/fit.py:58`, `bot/memory/__init__.py:34`, `bot/memory/earlier.py:34`, `bot/memory/lineback.py:35`, `bot/memory/store_lines.py:36`, `bot/modules/__init__.py:48`, `bot/modules/answers.py:77`, `bot/modules/attribute_said_again.py:94`, `bot/modules/memory_store.py:48`

## `bot/modules/saving_followup.py`

Imports: `bot/__init__.py:27`, `bot/memory/__init__.py:27`, `bot/memory/again.py:301`, `bot/memory/saving.py:27`, `bot/modules/__init__.py:29`, `bot/modules/answers.py:29`, `bot/understanding/__init__.py:28`

## `bot/modules/since_it_began.py`

Imports: `bot/__init__.py:21`, `bot/memory/__init__.py:21`, `bot/memory/when.py:21`

## `bot/modules/steps.py`

Imports: `bot/__init__.py:15`, `bot/memory/__init__.py:15`, `bot/memory/plan.py:15`

## `bot/modules/taking_asked_again.py`

Imports: `bot/__init__.py:23`, `bot/memory/__init__.py:23`, `bot/memory/taking.py:23`

## `bot/modules/talkto.py`

Imports: `bot/__init__.py:22`, `bot/compose/__init__.py:22`, `bot/compose/fallback_data.py:23`, `bot/compose/talkto.py:22`, `bot/compose/urgent.py:22`, `bot/knowledge/__init__.py:24`, `bot/knowledge/lookup.py:24`, `bot/modules/__init__.py:115`, `bot/modules/greeting.py:115`

## `bot/modules/the_detail_the_thread_adds.py`

Imports: `bot/__init__.py:16`, `bot/compose/__init__.py:16`, `bot/compose/fallback.py:16`, `bot/compose/thread_detail.py:16`

## `bot/modules/the_kind_one_line_up.py`

Imports: `bot/__init__.py:30`, `bot/compose/__init__.py:30`, `bot/compose/fallback.py:30`, `bot/compose/kept_nearby.py:30`

## `bot/modules/the_month_they_have.py`

Imports: `bot/__init__.py:18`, `bot/memory/__init__.py:18`, `bot/memory/outgoings.py:18`

## `bot/modules/the_shape_of_the_answer.py`

Imports: `bot/__init__.py:37`, `bot/compose/__init__.py:37`, `bot/compose/fallback.py:37`, `bot/compose/the_shape_without_the_fact.py:38`, `bot/modules/__init__.py:79`, `bot/modules/greeting.py:79`

## `bot/modules/the_trouble_it_has.py`

Imports: `bot/__init__.py:13`, `bot/compose/__init__.py:13`, `bot/compose/a_trouble.py:13`, `bot/compose/fallback.py:13`

## `bot/modules/the_trouble_named.py`

Imports: `bot/__init__.py:16`, `bot/compose/__init__.py:16`, `bot/compose/the_trouble_named.py:16`

## `bot/modules/the_wait_about_a_body.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/compose/a_wait_about_a_body.py:24`, `bot/compose/fallback.py:23`

## `bot/modules/their_life.py`

Imports: `bot/__init__.py:366`, `bot/compose/__init__.py:366`, `bot/compose/fallback.py:366`

## `bot/modules/their_pastime.py`

Imports: `bot/__init__.py:23`, `bot/compose/__init__.py:23`, `bot/compose/fallback.py:23`, `bot/compose/remark.py:25`, `bot/memory/__init__.py:154`, `bot/memory/ack.py:154`

## `bot/modules/their_taste.py`

Imports: `bot/__init__.py:27`, `bot/compose/__init__.py:27`, `bot/compose/fallback.py:27`, `bot/memory/__init__.py:29`, `bot/memory/obstacle.py:29`, `bot/modules/__init__.py:189`, `bot/modules/greeting.py:189`

## `bot/modules/theirperson.py`

Imports: `bot/__init__.py:20`, `bot/compose/__init__.py:20`, `bot/compose/fallback.py:20`, `bot/compose/theirperson.py:20`

## `bot/modules/thing_i_did_with.py`

Imports: `bot/__init__.py:190`, `bot/modules/__init__.py:190`, `bot/modules/memory_store.py:190`

## `bot/modules/trip_date_said_again.py`

Imports: `bot/__init__.py:25`, `bot/memory/__init__.py:25`, `bot/memory/facts.py:25`, `bot/memory/trips.py:26`, `bot/modules/__init__.py:27`, `bot/modules/memory_store.py:27`

## `bot/modules/understand.py`

Imports: `bot/__init__.py:2`, `bot/memory/__init__.py:2`, `bot/memory/recall.py:2`, `bot/memory/topics.py:21`, `bot/understanding/__init__.py:3`

## `bot/modules/urgent.py`

Imports: `bot/__init__.py:15`, `bot/compose/__init__.py:15`, `bot/compose/urgent.py:15`

## `bot/modules/ward.py`

Imports: `bot/__init__.py:15`, `bot/compose/__init__.py:15`, `bot/compose/ward.py:15`

## `bot/modules/what_decides_it.py`

Imports: `bot/__init__.py:20`, `bot/compose/__init__.py:20`, `bot/compose/what_decides_it.py:20`

## `bot/modules/what_they_said.py`

Imports: `bot/__init__.py:41`, `bot/memory/__init__.py:41`, `bot/memory/lineback.py:41`

## `bot/modules/where_it_answers.py`

Imports: `bot/__init__.py:26`, `bot/compose/__init__.py:26`, `bot/compose/fallback.py:26`, `bot/compose/findout.py:26`, `bot/compose/routes.py:26`, `bot/modules/__init__.py:108`, `bot/modules/greeting.py:108`

## `bot/modules/who_to_see.py`

Imports: `bot/__init__.py:16`, `bot/compose/__init__.py:16`, `bot/compose/who.py:16`

## `bot/modules/words.py`

Imports: `bot/__init__.py:18`, `bot/compose/__init__.py:18`, `bot/memory/__init__.py:19`

## `bot/modules/years_in.py`

Imports: `bot/__init__.py:17`, `bot/memory/__init__.py:17`, `bot/memory/years_in.py:17`

## `bot/runtime.py`

Imports: `bot/__init__.py:22`, `bot/modules/__init__.py:22`

## `bot/understanding/__init__.py`

Imports: `bot/__init__.py:22`, `bot/understanding/__init__.py:22`, `bot/understanding/attempts.py:22`, `bot/understanding/body.py:23`, `bot/understanding/breeds.py:24`, `bot/understanding/cars.py:25`, `bot/understanding/confusion.py:29`, `bot/understanding/kinds.py:25`, `bot/understanding/lexicon.py:25`, `bot/understanding/normalize.py:25`, `bot/understanding/references.py:25`, `bot/understanding/senses.py:25`, `bot/understanding/topics.py:25`

## `bot/understanding/attempts.py`

Imports: none

## `bot/understanding/body.py`

Imports: `bot/__init__.py:24`, `bot/understanding/__init__.py:24`, `bot/understanding/kinds.py:24`, `bot/understanding/lexicon.py:24`, `bot/understanding/normalize.py:25`, `bot/understanding/topics.py:309`

## `bot/understanding/breeds.py`

Imports: none

## `bot/understanding/car_glass.py`

Imports: none

## `bot/understanding/cars.py`

Imports: none

## `bot/understanding/confusion.py`

Imports: `bot/__init__.py:26`, `bot/understanding/__init__.py:26`, `bot/understanding/kinds.py:26`

## `bot/understanding/kinds.py`

Imports: `bot/__init__.py:12`, `bot/compose/__init__.py:380`, `bot/compose/ruledout.py:380`, `bot/memory/__init__.py:3734`, `bot/memory/facts.py:3734`, `bot/memory/meant.py:5813`, `bot/understanding/__init__.py:12`, `bot/understanding/lexicon.py:12`, `bot/understanding/normalize.py:13`, `bot/understanding/topics.py:6532`

## `bot/understanding/lexicon.py`

Imports: `bot/__init__.py:1987`, `bot/understanding/__init__.py:1987`, `bot/understanding/body.py:1987`, `bot/understanding/kinds.py:1987`, `bot/understanding/topics.py:1987`

## `bot/understanding/normalize.py`

Imports: `bot/__init__.py:20`, `bot/knowledge/__init__.py:670`, `bot/knowledge/geography.py:4504`, `bot/knowledge/lookup.py:671`, `bot/understanding/__init__.py:20`, `bot/understanding/kinds.py:1962`, `bot/understanding/lexicon.py:20`

## `bot/understanding/references.py`

Imports: `bot/__init__.py:13`, `bot/understanding/__init__.py:13`, `bot/understanding/breeds.py:347`, `bot/understanding/topics.py:13`

## `bot/understanding/senses.py`

Imports: none

## `bot/understanding/topics.py`

Imports: `bot/__init__.py:17`, `bot/understanding/__init__.py:17`, `bot/understanding/body.py:18`, `bot/understanding/kinds.py:19`, `bot/understanding/lexicon.py:17`, `bot/understanding/normalize.py:20`
