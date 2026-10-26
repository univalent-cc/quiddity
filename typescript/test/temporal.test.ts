// Temporal objects, where the runtime has them (Node 24 with
// --harmony-temporal: `npm run test:temporal`).

import assert from "node:assert/strict";
import { describe, test } from "node:test";

import * as q from "../dist/index.js";

const T = (globalThis as { Temporal?: any }).Temporal;

describe("Temporal", { skip: T === undefined ? "this runtime has no Temporal" : false }, () => {
  test("instants, in any time zone", () => {
    const i = T.Instant.from("2024-01-01T00:00:00Z");
    const d = q.digest(new Date("2024-01-01T00:00:00Z"));
    assert.ok(q.digest(i).equals(d));
    assert.ok(q.digest(i.toZonedDateTimeISO("Asia/Tokyo")).equals(d));
    assert.ok(q.instant().digest(i).equals(q.instant().digest("2024-01-01T09:00:00+09:00")));
  });

  test("durations, dates and times", () => {
    assert.ok(q.digest(T.Duration.from("PT1H0.000005S")).equals(q.digest(new q.Duration(3_600_000_005_000n))));
    assert.ok(q.duration().digest(T.Duration.from("PT1H30M")).equals(q.duration().digest("PT90M")));
    assert.ok(q.digest(T.PlainDate.from("2024-01-01")).equals(q.digest("2024-01-01")));
    assert.ok(q.digest(T.PlainDateTime.from("2024-01-01T00:00")).equals(q.digest(T.Instant.from("2024-01-01T00:00Z"))));
    assert.ok(q.digest(T.PlainTime.from("09:30:00.5")).equals(q.digest(new q.Duration(34_200_500_000_000n))));
    assert.ok(q.instant().digest(T.PlainDateTime.from("2024-01-01T00:00")).equals(q.instant().digest("2024-01-01T00:00:00")));
    assert.ok(q.duration().digest(T.PlainTime.from("01:30")).equals(q.duration().digest("PT90M")));
    assert.ok(q.digest(T.Duration.from("P1M")).equals(q.digest(new q.Duration(2_629_746_000_000_000n))));
  });
});
