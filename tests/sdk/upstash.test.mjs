import assert from 'node:assert/strict'
import test from 'node:test'
import { Redis } from '@upstash/redis'

const url = process.env.RRB_TEST_URL ?? 'http://127.0.0.1:7777'
const token = process.env.RRB_TOKEN

if (!token) {
  throw new Error('RRB_TOKEN is required for SDK compatibility tests')
}

const redis = new Redis({
  url,
  token,
})

test('Single commands work through @upstash/redis', async () => {
  await redis.del('sdk:hello')

  const setResult = await redis.set('sdk:hello', 'world')
  assert.equal(setResult, 'OK')

  const value = await redis.get('sdk:hello')
  assert.equal(value, 'world')

  const ping = await redis.ping()
  assert.equal(ping, 'PONG')
})

test('Pipeline works through @upstash/redis', async () => {
  await redis.del('sdk:p1')

  const pipeline = redis.pipeline()
  pipeline.set('sdk:p1', 'v1')
  pipeline.get('sdk:p1')

  const result = await pipeline.exec()

  assert.deepEqual(result, ['OK', 'v1'])
})

test('Multi exec works through @upstash/redis', async () => {
  await redis.del('sdk:counter')

  const tx = redis.multi()
  tx.set('sdk:counter', '1')
  tx.incr('sdk:counter')

  const result = await tx.exec()

  assert.deepEqual(result, ['OK', 2])
})

test('Pipeline keeps per-command Redis errors when keepErrors is true', async () => {
  await redis.del('sdk:num')

  const pipeline = redis.pipeline()
  pipeline.set('sdk:num', 'not-a-number')
  pipeline.incr('sdk:num')

  const result = await pipeline.exec({ keepErrors: true })

  assert.equal(result.length, 2)

  const first = result[0]
  const second = result[1]

  assert.equal(first.result, 'OK')
  assert.equal(first.error, undefined)

  assert.equal(second.result, undefined)
  assert.ok(
    second.error,
    `Expected second pipeline item to contain an error, got: ${JSON.stringify(second)}`
  )

  assert.match(String(second.error), /integer|number|ERR|value/i)
})

test('Null and numeric responses match @upstash/redis expectations', async () => {
  await redis.del('sdk:missing')
  assert.equal(await redis.get('sdk:missing'), null)

  await redis.set('sdk:n', '1')
  assert.equal(await redis.incr('sdk:n'), 2)
})
