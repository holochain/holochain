import { Orchestrator, Config } from "@holochain/tryorama";
import { cond } from "lodash";

const orchestrator = new Orchestrator();

export const simpleConfig = {
  alice: Config.dna("../sourcechainsize.dna.gz", null),
  bobbo: Config.dna("../sourcechainsize.dna.gz", null),
};

async function commitEntry(t, conductor, playerName, char) {
  const entry = `${char}`;
  const hash = await conductor.call(playerName, "sourcechainsize", "create_entry", entry);
  t.ok(hash);

  const result = await conductor.call(playerName, 'sourcechainsize', 'get_entry', hash);
  t.ok(result);
}

orchestrator.registerScenario("many commits", async (s, t) => {
  const { conductor } = await s.players({
    conductor: Config.gen(simpleConfig),
  });
  await conductor.spawn();

  const NUMBER_OF_ENTRIES_TO_COMMIT = 269;
  for (let i = 0; i < NUMBER_OF_ENTRIES_TO_COMMIT; i++) {
    await commitEntry(t, conductor, 'alice', String.fromCharCode('a'.charCodeAt(0) + i));
  }

  const stateDump = await conductor.stateDump('alice');

  t.equal(stateDump.length - 4, NUMBER_OF_ENTRIES_TO_COMMIT)
});

orchestrator.run();
