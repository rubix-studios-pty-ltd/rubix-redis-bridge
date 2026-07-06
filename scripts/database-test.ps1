$ErrorActionPreference = "Stop"

$BaseUrl = "http://127.0.0.1:7777"
$TestPrefix = "kv:bridge:test:$([guid]::NewGuid().ToString("N"))"

if ([string]::IsNullOrWhiteSpace($env:RRB_TOKEN)) {
  throw "RRB_TOKEN is not set in this PowerShell session."
}

$Headers = @{
  Authorization = "Bearer $env:RRB_TOKEN"
  "Content-Type" = "application/json"
}

$Results = New-Object System.Collections.Generic.List[object]

function Add-TestResult {
  param(
    [string] $Name,
    [string] $Status,
    [object] $Value = $null,
    [string] $ErrorMessage = $null
  )

  $Results.Add([pscustomobject]@{
    Test = $Name
    Status = $Status
    Value = if ($null -eq $Value) { "" } else { ($Value | ConvertTo-Json -Compress -Depth 20) }
    Error = $ErrorMessage
  }) | Out-Null
}

function Get-RrbResult {
  param([object] $Response)

  if ($null -eq $Response) {
    return $null
  }

  if ($Response.PSObject.Properties.Name -contains "result") {
    return $Response.result
  }

  return $Response
}

function Invoke-RrbCommand {
  param(
    [Parameter(Mandatory = $true)]
    [object[]] $Command
  )

  $Body = $Command | ConvertTo-Json -Compress -Depth 100

  Invoke-RestMethod `
    -Uri "$BaseUrl/" `
    -Method POST `
    -Headers $Headers `
    -Body $Body
}

function Invoke-RrbPipeline {
  param(
    [Parameter(Mandatory = $true)]
    [object[]] $Pipeline
  )

  $Body = $Pipeline | ConvertTo-Json -Compress -Depth 100

  Invoke-RestMethod `
    -Uri "$BaseUrl/pipeline" `
    -Method POST `
    -Headers $Headers `
    -Body $Body
}

function Invoke-Expect {
  param(
    [Parameter(Mandatory = $true)]
    [string] $Name,

    [Parameter(Mandatory = $true)]
    [scriptblock] $Script,

    [scriptblock] $Assert = $null
  )

  try {
    $Response = & $Script
    $Result = Get-RrbResult $Response

    if ($null -ne $Assert) {
      $Ok = & $Assert $Result $Response
      if (-not $Ok) {
        Add-TestResult -Name $Name -Status "FAIL" -Value $Response -ErrorMessage "Assertion failed"
        return
      }
    }

    Add-TestResult -Name $Name -Status "PASS" -Value $Response
  } catch {
    Add-TestResult -Name $Name -Status "FAIL" -ErrorMessage $_.Exception.Message
  }
}

function Invoke-ExpectStatus {
  param(
    [Parameter(Mandatory = $true)]
    [string] $Name,

    [Parameter(Mandatory = $true)]
    [scriptblock] $Script,

    [Parameter(Mandatory = $true)]
    [int] $ExpectedStatusCode
  )

  try {
    & $Script | Out-Null
    Add-TestResult -Name $Name -Status "FAIL" -ErrorMessage "Expected HTTP $ExpectedStatusCode but request succeeded"
  } catch {
    $StatusCode = $null

    if ($_.Exception.Response -and $_.Exception.Response.StatusCode) {
      $StatusCode = [int]$_.Exception.Response.StatusCode
    }

    if ($StatusCode -eq $ExpectedStatusCode) {
      Add-TestResult -Name $Name -Status "PASS" -Value @{ status = $StatusCode }
    } else {
      Add-TestResult -Name $Name -Status "FAIL" -Value @{ status = $StatusCode } -ErrorMessage $_.Exception.Message
    }
  }
}

Write-Host ""
Write-Host "Testing Rubix Redis Bridge via HTTP"
Write-Host "Base URL: $BaseUrl"
Write-Host "Key prefix: $TestPrefix"
Write-Host ""

Invoke-Expect `
  -Name "healthz returns ok" `
  -Script {
    Invoke-RestMethod -Uri "$BaseUrl/healthz" -Method GET
  }

Invoke-Expect `
  -Name "PING" `
  -Script {
    Invoke-RrbCommand @("PING", "PONG")
  } `
  -Assert {
    param($Result, $Response)
    "$Result" -eq "PONG"
  }

Invoke-Expect `
  -Name "SET with EX" `
  -Script {
    Invoke-RrbCommand @("SET", "${TestPrefix}:string", "ok", "EX", "60")
  } `
  -Assert {
    param($Result, $Response)
    "$Result" -eq "OK"
  }

Invoke-Expect `
  -Name "GET string" `
  -Script {
    Invoke-RrbCommand @("GET", "${TestPrefix}:string")
  } `
  -Assert {
    param($Result, $Response)
    "$Result" -eq "ok"
  }

Invoke-Expect `
  -Name "EXISTS string" `
  -Script {
    Invoke-RrbCommand @("EXISTS", "${TestPrefix}:string")
  } `
  -Assert {
    param($Result, $Response)
    [int]$Result -eq 1
  }

Invoke-Expect `
  -Name "TTL string" `
  -Script {
    Invoke-RrbCommand @("TTL", "${TestPrefix}:string")
  } `
  -Assert {
    param($Result, $Response)
    [int]$Result -gt 0
  }

Invoke-Expect `
  -Name "DEL counter before INCR" `
  -Script {
    Invoke-RrbCommand @("DEL", "${TestPrefix}:counter")
  }

Invoke-Expect `
  -Name "INCR counter first" `
  -Script {
    Invoke-RrbCommand @("INCR", "${TestPrefix}:counter")
  } `
  -Assert {
    param($Result, $Response)
    [int]$Result -eq 1
  }

Invoke-Expect `
  -Name "INCR counter second" `
  -Script {
    Invoke-RrbCommand @("INCR", "${TestPrefix}:counter")
  } `
  -Assert {
    param($Result, $Response)
    [int]$Result -eq 2
  }

Invoke-Expect `
  -Name "DECR counter" `
  -Script {
    Invoke-RrbCommand @("DECR", "${TestPrefix}:counter")
  } `
  -Assert {
    param($Result, $Response)
    [int]$Result -eq 1
  }

Invoke-Expect `
  -Name "GET counter" `
  -Script {
    Invoke-RrbCommand @("GET", "${TestPrefix}:counter")
  } `
  -Assert {
    param($Result, $Response)
    "$Result" -eq "1"
  }

Invoke-Expect `
  -Name "DEL hash before HSET" `
  -Script {
    Invoke-RrbCommand @("DEL", "${TestPrefix}:hash")
  }

Invoke-Expect `
  -Name "HSET multiple fields" `
  -Script {
    Invoke-RrbCommand @("HSET", "${TestPrefix}:hash", "name", "database", "status", "ok", "count", "3")
  } `
  -Assert {
    param($Result, $Response)
    [int]$Result -ge 1
  }

Invoke-Expect `
  -Name "HGET field" `
  -Script {
    Invoke-RrbCommand @("HGET", "${TestPrefix}:hash", "name")
  } `
  -Assert {
    param($Result, $Response)
    "$Result" -eq "database"
  }

Invoke-Expect `
  -Name "HMGET existing and missing fields" `
  -Script {
    Invoke-RrbCommand @("HMGET", "${TestPrefix}:hash", "name", "status", "missing")
  } `
  -Assert {
    param($Result, $Response)
    $Result.Count -eq 3 -and "$($Result[0])" -eq "database" -and "$($Result[1])" -eq "ok"
  }

Invoke-Expect `
  -Name "HGETALL hash" `
  -Script {
    Invoke-RrbCommand @("HGETALL", "${TestPrefix}:hash")
  } `
  -Assert {
    param($Result, $Response)
    ($Result | ConvertTo-Json -Compress -Depth 20).Contains("database")
  }

Invoke-Expect `
  -Name "HDEL field" `
  -Script {
    Invoke-RrbCommand @("HDEL", "${TestPrefix}:hash", "status")
  } `
  -Assert {
    param($Result, $Response)
    [int]$Result -eq 1
  }

Invoke-Expect `
  -Name "HGET deleted field returns null" `
  -Script {
    Invoke-RrbCommand @("HGET", "${TestPrefix}:hash", "status")
  } `
  -Assert {
    param($Result, $Response)
    $null -eq $Result
  }

Invoke-Expect `
  -Name "DEL zset before ZINCRBY" `
  -Script {
    Invoke-RrbCommand @("DEL", "${TestPrefix}:zset")
  }

Invoke-Expect `
  -Name "ZINCRBY first member" `
  -Script {
    Invoke-RrbCommand @("ZINCRBY", "${TestPrefix}:zset", "1", "client-a")
  } `
  -Assert {
    param($Result, $Response)
    [double]$Result -eq 1
  }

Invoke-Expect `
  -Name "ZINCRBY same member again" `
  -Script {
    Invoke-RrbCommand @("ZINCRBY", "${TestPrefix}:zset", "2", "client-a")
  } `
  -Assert {
    param($Result, $Response)
    [double]$Result -eq 3
  }

Invoke-Expect `
  -Name "DEL eval key" `
  -Script {
    Invoke-RrbCommand @("DEL", "${TestPrefix}:eval")
  }

Invoke-Expect `
  -Name "EVAL INCR script" `
  -Script {
    Invoke-RrbCommand @("EVAL", "return redis.call('INCR', KEYS[1])", 1, "${TestPrefix}:eval")
  } `
  -Assert {
    param($Result, $Response)
    [int]$Result -eq 1
  }

Invoke-Expect `
  -Name "GET eval key" `
  -Script {
    Invoke-RrbCommand @("GET", "${TestPrefix}:eval")
  } `
  -Assert {
    param($Result, $Response)
    "$Result" -eq "1"
  }

$LoadedSha = $null

Invoke-Expect `
  -Name "SCRIPT LOAD" `
  -Script {
    Invoke-RrbCommand @("SCRIPT", "LOAD", "return redis.call('INCR', KEYS[1])")
  } `
  -Assert {
    param($Result, $Response)
    $script:LoadedSha = "$Result"
    $script:LoadedSha.Length -ge 40
  }

Invoke-Expect `
  -Name "EVALSHA loaded script" `
  -Script {
    Invoke-RrbCommand @("EVALSHA", $LoadedSha, 1, "${TestPrefix}:evalsha")
  } `
  -Assert {
    param($Result, $Response)
    [int]$Result -eq 1
  }

Invoke-Expect `
  -Name "SCRIPT EXISTS loaded script" `
  -Script {
    Invoke-RrbCommand @("SCRIPT", "EXISTS", $LoadedSha)
  } `
  -Assert {
    param($Result, $Response)
    $Result.Count -eq 1 -and [int]$Result[0] -eq 1
  }

Invoke-Expect `
  -Name "Pipeline mixed commands" `
  -Script {
    Invoke-RrbPipeline @(
      @("DEL", "${TestPrefix}:pipeline"),
      @("SET", "${TestPrefix}:pipeline", "1"),
      @("INCR", "${TestPrefix}:pipeline"),
      @("GET", "${TestPrefix}:pipeline"),
      @("TTL", "${TestPrefix}:pipeline")
    )
  } `
  -Assert {
    param($Result, $Response)

    $Json = $Response | ConvertTo-Json -Compress -Depth 50

    $Json.Contains("OK") -and $Json.Contains("2")
  }

Invoke-Expect `
  -Name "Pipeline mixed success and Redis error" `
  -Script {
    Invoke-RrbPipeline @(
      @("SET", "${TestPrefix}:type", "string-value"),
      @("HGETALL", "${TestPrefix}:type"),
      @("GET", "${TestPrefix}:type")
    )
  }

Invoke-Expect `
  -Name "SET unicode payload" `
  -Script {
    Invoke-RrbCommand @("SET", "${TestPrefix}:unicode", "Database ✅ 資料庫")
  } `
  -Assert {
    param($Result, $Response)
    "$Result" -eq "OK"
  }

Invoke-Expect `
  -Name "GET unicode payload" `
  -Script {
    Invoke-RrbCommand @("GET", "${TestPrefix}:unicode")
  } `
  -Assert {
    param($Result, $Response)
    "$Result" -eq "Database ✅ 資料庫"
  }


function Invoke-ExpectStatusAny {
  param(
    [Parameter(Mandatory = $true)]
    [string] $Name,

    [Parameter(Mandatory = $true)]
    [scriptblock] $Script,

    [Parameter(Mandatory = $true)]
    [int[]] $ExpectedStatusCodes
  )

  try {
    & $Script | Out-Null
    Add-TestResult -Name $Name -Status "FAIL" -ErrorMessage "Expected HTTP $($ExpectedStatusCodes -join ', ') but request succeeded"
  } catch {
    $StatusCode = $null

    if ($_.Exception.Response -and $_.Exception.Response.StatusCode) {
      $StatusCode = [int]$_.Exception.Response.StatusCode
    }

    if ($ExpectedStatusCodes -contains $StatusCode) {
      Add-TestResult -Name $Name -Status "PASS" -Value @{ status = $StatusCode }
    } else {
      Add-TestResult -Name $Name -Status "FAIL" -Value @{ status = $StatusCode } -ErrorMessage $_.Exception.Message
    }
  }
}

# Bridge security and validation checks.
Invoke-ExpectStatusAny `
  -Name "Denied command FLUSHALL blocked by bridge" `
  -ExpectedStatusCodes @(400, 403) `
  -Script {
    Invoke-RrbCommand @("FLUSHALL")
  }

Invoke-ExpectStatus `
  -Name "Unauthorized token returns 401" `
  -ExpectedStatusCode 401 `
  -Script {
    $BadHeaders = @{
      Authorization = "Bearer wrong-token"
      "Content-Type" = "application/json"
    }

    Invoke-RestMethod `
      -Uri "$BaseUrl/" `
      -Method POST `
      -Headers $BadHeaders `
      -Body '["PING"]'
  }

Invoke-ExpectStatus `
  -Name "Missing auth returns 401" `
  -ExpectedStatusCode 401 `
  -Script {
    Invoke-RestMethod `
      -Uri "$BaseUrl/" `
      -Method POST `
      -Headers @{ "Content-Type" = "application/json" } `
      -Body '["PING"]'
  }

Invoke-ExpectStatus `
  -Name "Malformed JSON returns 400" `
  -ExpectedStatusCode 400 `
  -Script {
    Invoke-RestMethod `
      -Uri "$BaseUrl/" `
      -Method POST `
      -Headers $Headers `
      -Body 'not-json'
  }

Invoke-ExpectStatus `
  -Name "Non-array JSON returns 400" `
  -ExpectedStatusCode 400 `
  -Script {
    Invoke-RestMethod `
      -Uri "$BaseUrl/" `
      -Method POST `
      -Headers $Headers `
      -Body '{"command":"PING"}'
  }

$TooManyArgs = @("HSET", "${TestPrefix}:too-many")
1..300 | ForEach-Object {
  $TooManyArgs += "field$_"
  $TooManyArgs += "value$_"
}

Invoke-ExpectStatus `
  -Name "Too many command args rejected" `
  -ExpectedStatusCode 400 `
  -Script {
    Invoke-RrbCommand $TooManyArgs
  }

# Cleanup.
Invoke-Expect `
  -Name "Cleanup test keys" `
  -Script {
    Invoke-RrbCommand @(
      "DEL",
      "${TestPrefix}:string",
      "${TestPrefix}:counter",
      "${TestPrefix}:hash",
      "${TestPrefix}:zset",
      "${TestPrefix}:eval",
      "${TestPrefix}:evalsha",
      "${TestPrefix}:pipeline",
      "${TestPrefix}:type",
      "${TestPrefix}:unicode",
      "${TestPrefix}:too-many"
    )
  }

if (-not [string]::IsNullOrWhiteSpace($env:RRB_METRICS_TOKEN)) {
  Invoke-Expect `
    -Name "Metrics endpoint includes bridge metrics" `
    -Script {
      $MetricsHeaders = @{
        Authorization = "Bearer $env:RRB_METRICS_TOKEN"
      }

      $Response = Invoke-WebRequest `
        -Uri "$BaseUrl/metrics" `
        -Headers $MetricsHeaders

      $Response.Content
    } `
    -Assert {
      param($Result, $Response)
      $Result.Contains("rrb_redis_operations_total") -and
      $Result.Contains("rrb_configured_targets")
    }
} else {
  Add-TestResult -Name "Metrics endpoint skipped" -Status "SKIP" -ErrorMessage "RRB_METRICS_TOKEN is not set"
}

Write-Host ""
Write-Host "Results"
Write-Host "-------"

$Results | Format-Table -AutoSize

$Failed = $Results | Where-Object { $_.Status -eq "FAIL" }

if ($Failed.Count -gt 0) {
  Write-Host ""
  Write-Host "Failed tests"
  Write-Host "------------"
  $Failed | Format-List

  throw "$($Failed.Count) bridge to running database test(s) failed."
}

Write-Host ""
Write-Host "All bridge to database tests passed."