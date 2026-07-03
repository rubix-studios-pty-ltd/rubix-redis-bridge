import assert from 'node:assert/strict'
import test from 'node:test'
import { Ratelimit } from '@upstash/ratelimit'
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

async function rawCommand(command) {
  const response = await fetch(url, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify(command),
  })

  const body = await response.json().catch(() => null)

  return {
    status: response.status,
    body,
    result: body?.result,
    error: body?.error,
  }
}

test('Single string commands work through @upstash/redis', async () => {
  await redis.del('sdk:hello')

  const setResult = await redis.set('sdk:hello', 'world')
  assert.equal(setResult, 'OK')

  const value = await redis.get('sdk:hello')
  assert.equal(value, 'world')

  const exists = await redis.exists('sdk:hello')
  assert.equal(exists, 1)

  const deleted = await redis.del('sdk:hello')
  assert.equal(deleted, 1)

  const missing = await redis.get('sdk:hello')
  assert.equal(missing, null)
})

test('PING works through @upstash/redis', async () => {
  const ping = await redis.ping()

  assert.equal(ping, 'PONG')
})

test('Numeric commands work through @upstash/redis', async () => {
  await redis.del('sdk:number')

  const setResult = await redis.set('sdk:number', '1')
  assert.equal(setResult, 'OK')

  const incremented = await redis.incr('sdk:number')
  assert.equal(incremented, 2)

  const decremented = await redis.decr('sdk:number')
  assert.equal(decremented, 1)
})

test('TTL and EXPIRE commands work through @upstash/redis', async () => {
  await redis.del('sdk:ttl')

  const setResult = await redis.set('sdk:ttl', '1', { ex: 60 })
  assert.equal(setResult, 'OK')

  const ttl = await redis.ttl('sdk:ttl')
  assert.ok(ttl > 0)
  assert.ok(ttl <= 60)

  const expireResult = await redis.expire('sdk:ttl', 120)
  assert.equal(expireResult, 1)

  const updatedTtl = await redis.ttl('sdk:ttl')
  assert.ok(updatedTtl > 60)
  assert.ok(updatedTtl <= 120)
})

test('Missing keys return null through @upstash/redis', async () => {
  await redis.del('sdk:missing')
  await redis.del('sdk:missing-hash')

  assert.equal(await redis.get('sdk:missing'), null)
  assert.equal(await redis.hget('sdk:missing-hash', 'field'), null)
})

test('Hash commands work through @upstash/redis', async () => {
  await redis.del('sdk:h')

  const hsetResult = await redis.hset('sdk:h', {
    a: 'one',
    b: 'two',
  })

  assert.equal(hsetResult, 2)

  const hgetResult = await redis.hget('sdk:h', 'a')
  assert.equal(hgetResult, 'one')

  const hgetallResult = await redis.hgetall('sdk:h')
  assert.deepEqual(hgetallResult, {
    a: 'one',
    b: 'two',
  })

  const hdelResult = await redis.hdel('sdk:h', 'a')
  assert.equal(hdelResult, 1)

  const deletedField = await redis.hget('sdk:h', 'a')
  assert.equal(deletedField, null)

  const remaining = await redis.hgetall('sdk:h')
  assert.deepEqual(remaining, {
    b: 'two',
  })
})

test('HMGET works through raw Upstash-style command shape', async () => {
  await redis.del('sdk:hmget')

  const hsetResult = await redis.hset('sdk:hmget', {
    a: 'one',
    b: 'two',
  })

  assert.equal(hsetResult, 2)

  const result = await rawCommand(['HMGET', 'sdk:hmget', 'a', 'b', 'missing'])

  assert.equal(result.status, 200)
  assert.deepEqual(result.result, ['one', 'two', null])
})

test('Pipeline works through @upstash/redis', async () => {
  await redis.del('sdk:pipeline')

  const pipeline = redis.pipeline()

  pipeline.set('sdk:pipeline', 'v1')
  pipeline.get('sdk:pipeline')
  pipeline.exists('sdk:pipeline')
  pipeline.incr('sdk:pipeline-counter')
  pipeline.decr('sdk:pipeline-counter')

  const result = await pipeline.exec()

  assert.deepEqual(result, ['OK', 'v1', 1, 1, 0])
})

test('Pipeline keeps per-command Redis errors when keepErrors is true', async () => {
  await redis.del('sdk:num')

  const pipeline = redis.pipeline()

  pipeline.set('sdk:num', 'not-a-number')
  pipeline.incr('sdk:num')
  pipeline.get('sdk:num')

  const result = await pipeline.exec({ keepErrors: true })

  assert.equal(result.length, 3)

  const first = result[0]
  const second = result[1]
  const third = result[2]

  assert.equal(first.result, 'OK')
  assert.equal(first.error, undefined)

  assert.equal(second.result, undefined)
  assert.ok(
    second.error,
    `Expected second pipeline item to contain an error, got: ${JSON.stringify(second)}`
  )
  assert.match(String(second.error), /integer|number|ERR|value/i)

  assert.equal(third.result, 'not-a-number')
  assert.equal(third.error, undefined)
})

test('Multi exec works through @upstash/redis', async () => {
  await redis.del('sdk:counter')

  const tx = redis.multi()

  tx.set('sdk:counter', '1')
  tx.incr('sdk:counter')
  tx.decr('sdk:counter')
  tx.get('sdk:counter')

  const result = await tx.exec()

  assert.deepEqual(result, ['OK', 2, 1, 1])
})

test('Raw command endpoint matches Upstash-style JSON response', async () => {
  await redis.del('sdk:raw')

  const setResult = await rawCommand(['SET', 'sdk:raw', 'ok'])
  assert.equal(setResult.status, 200)
  assert.equal(setResult.result, 'OK')

  const getResult = await rawCommand(['GET', 'sdk:raw'])
  assert.equal(getResult.status, 200)
  assert.equal(getResult.result, 'ok')
})

test('Unauthorized requests are rejected', async () => {
  const response = await fetch(url, {
    method: 'POST',
    headers: {
      authorization: 'Bearer wrong-token',
      'content-type': 'application/json',
    },
    body: JSON.stringify(['PING']),
  })

  assert.equal(response.status, 401)
})

test('Dangerous commands are rejected before Redis execution', async () => {
  const result = await rawCommand(['FCALL', 'some_function', 0])

  assert.equal(result.status, 400)
  assert.ok(result.error)
  assert.match(result.error, /hard-denied|not allowed/i)
})

test('Connection-state commands are rejected before Redis execution', async () => {
  const result = await rawCommand(['SELECT', '1'])

  assert.equal(result.status, 400)
  assert.ok(result.error)
})

test('Upstash ratelimit fixed window works through the bridge', async () => {
  if (process.env.RRB_UPSTASH_RATELIMIT !== 'true') {
    test.skip('RRB_UPSTASH_RATELIMIT=true is required')
  }

  const prefix = `sdk:ratelimit:${Date.now()}:fixed`
  const identifier = 'user-1'

  const ratelimit = new Ratelimit({
    redis,
    limiter: Ratelimit.fixedWindow(2, '10 s'),
    prefix,
    analytics: false,
  })

  const first = await ratelimit.limit(identifier)
  assert.equal(first.success, true)
  assert.equal(first.limit, 2)
  assert.equal(first.remaining, 1)

  const second = await ratelimit.limit(identifier)
  assert.equal(second.success, true)
  assert.equal(second.limit, 2)
  assert.equal(second.remaining, 0)

  const third = await ratelimit.limit(identifier)
  assert.equal(third.success, false)
  assert.equal(third.limit, 2)
  assert.equal(third.remaining, 0)
})

test('Upstash ratelimit EVALSHA fallback to EVAL works after SCRIPT FLUSH', async () => {
  if (process.env.RRB_UPSTASH_RATELIMIT !== 'true') {
    test.skip('RRB_UPSTASH_RATELIMIT=true is required')
  }

  const flush = await rawCommand(['SCRIPT', 'FLUSH'])
  assert.equal(flush.status, 200)

  const prefix = `sdk:ratelimit:${Date.now()}:fallback`
  const identifier = 'user-1'

  const ratelimit = new Ratelimit({
    redis,
    limiter: Ratelimit.fixedWindow(1, '10 s'),
    prefix,
    analytics: false,
  })

  const first = await ratelimit.limit(identifier)
  assert.equal(first.success, true)
  assert.equal(first.limit, 1)
  assert.equal(first.remaining, 0)

  const second = await ratelimit.limit(identifier)
  assert.equal(second.success, false)
  assert.equal(second.limit, 1)
  assert.equal(second.remaining, 0)
})
