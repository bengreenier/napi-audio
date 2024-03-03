import { test } from "uvu";

import { Decoder, setNativeTracing } from "../index.js";

setNativeTracing(true);

test("Can be allocated", () => {
  const decoder = new Decoder({
    enableGapless: false,
  });

  decoder.close();
});

test.run();
