import assert from 'node:assert/strict'
import test from 'node:test'
import { Ratelimit } from '@upstash/ratelimit'
import { Redis } from '@upstash/redis'

const url = process.env.RRB_TEST_URL ?? 'http://127.0.0.1:7777'
const token = process.env.RRB_TOKEN

if (!token) {
  throw new Error('RRB_TOKEN is required for tests')
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

function policy(result) {
  return (
    result.status === 400 &&
    typeof result.error === 'string' &&
    /hard-denied|not allowed|blocked|policy/i.test(result.error)
  )
}

async function ratelimitDisabled(t) {
  const command = await rawCommand(['EVAL', 'return 1', 0])

  if (command.status === 200) {
    return false
  }

  if (policy(command)) {
    t.skip('Bridge is running without Upstash ratelimit')
    return true
  }

  assert.fail(`Unexpected EVAL probe failure: ${JSON.stringify(command.body)}`)
}

async function scriptFlushDisabled(t) {
  const command = await rawCommand(['SCRIPT', 'FLUSH'])

  if (command.status === 200) {
    return false
  }

  if (policy(command)) {
    t.skip('Bridge is running without SCRIPT FLUSH')
    return true
  }

  assert.fail(`Unexpected SCRIPT FLUSH failure: ${JSON.stringify(command.body)}`)
}

test('Test(@upstash/redis): single string commands', async () => {
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

test('Test(@upstash/redis): ping/pong request received', async () => {
  const ping = await redis.ping()

  assert.equal(ping, 'PONG')
})

test('Test(@upstash/redis): numeric commands', async () => {
  await redis.del('sdk:number')

  const setResult = await redis.set('sdk:number', '1')
  assert.equal(setResult, 'OK')

  const incremented = await redis.incr('sdk:number')
  assert.equal(incremented, 2)

  const decremented = await redis.decr('sdk:number')
  assert.equal(decremented, 1)
})

test('Test(@upstash/redis): ttl and expire commands', async () => {
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

test('Test(@upstash/redis): missing keys return null', async () => {
  await redis.del('sdk:missing')
  await redis.del('sdk:missing-hash')

  assert.equal(await redis.get('sdk:missing'), null)
  assert.equal(await redis.hget('sdk:missing-hash', 'field'), null)
})

test('Test(@upstash/redis): hash commands', async () => {
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

test('Test(@upstash/redis): hmget commands', async () => {
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

test('Test(@upstash/redis): pipeline commands', async () => {
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

test('Test(@upstash/redis): pipeline per-command errors', async () => {
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

test('Test(@upstash/redis): multi exec commands', async () => {
  await redis.del('sdk:counter')

  const tx = redis.multi()

  tx.set('sdk:counter', '1')
  tx.incr('sdk:counter')
  tx.decr('sdk:counter')
  tx.get('sdk:counter')

  const result = await tx.exec()

  assert.deepEqual(result, ['OK', 2, 1, 1])
})

test('Test(@upstash/redis): command endpoint matches upstash response', async () => {
  await redis.del('sdk:raw')

  const setResult = await rawCommand(['SET', 'sdk:raw', 'ok'])
  assert.equal(setResult.status, 200)
  assert.equal(setResult.result, 'OK')

  const getResult = await rawCommand(['GET', 'sdk:raw'])
  assert.equal(getResult.status, 200)
  assert.equal(getResult.result, 'ok')
})

test('Test(@upstash/redis): unauthorized requests rejected', async () => {
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

test('Test(@upstash/redis): dangerous commands rejected before execution', async () => {
  const result = await rawCommand(['FCALL', 'some_function', 0])

  assert.equal(result.status, 400)
  assert.ok(result.error)
  assert.match(result.error, /hard-denied|not allowed/i)
})

test('Test(@upstash/redis): connection-state commands are rejected before execution', async () => {
  const result = await rawCommand(['SELECT', '1'])

  assert.equal(result.status, 400)
  assert.ok(result.error)
})

test('Test(@upstash/redis): upstash ratelimit fixed window', async (t) => {
  if (await ratelimitDisabled(t)) {
    return
  }

  const prefix = `sdk:ratelimit:${Date.now()}:fixed`
  const identifier = 'user-1'

  const limiter = new Ratelimit({
    redis,
    limiter: Ratelimit.fixedWindow(2, '10 s'),
    prefix,
    analytics: false,
  })

  const first = await limiter.limit(identifier)
  assert.equal(first.success, true)
  assert.equal(first.limit, 2)
  assert.equal(first.remaining, 1)

  const second = await limiter.limit(identifier)
  assert.equal(second.success, true)
  assert.equal(second.limit, 2)
  assert.equal(second.remaining, 0)

  const third = await limiter.limit(identifier)
  assert.equal(third.success, false)
  assert.equal(third.limit, 2)
  assert.equal(third.remaining, 0)
})

test('Test(@upstash/redis): upstash ratelimit fallback', async (t) => {
  if (await scriptFlushDisabled(t)) {
    return
  }

  const prefix = `sdk:ratelimit:${Date.now()}:fallback`
  const identifier = 'user-1'

  const limiter = new Ratelimit({
    redis,
    limiter: Ratelimit.fixedWindow(1, '10 s'),
    prefix,
    analytics: false,
  })

  const first = await limiter.limit(identifier)
  assert.equal(first.success, true)
  assert.equal(first.limit, 1)
  assert.equal(first.remaining, 0)

  const second = await limiter.limit(identifier)
  assert.equal(second.success, false)
  assert.equal(second.limit, 1)
  assert.equal(second.remaining, 0)
})
