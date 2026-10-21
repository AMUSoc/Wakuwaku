# A weather widget for Wakuwaku's island: the sky and the temperature in a
# city, and the chance of rain in the next 12 hours, from Open-Meteo (no key
# needed), every 10 minutes. The city can be named in English or Chinese;
# Taiwan's cities and counties are known by their own names (台北, 臺中,
# 新竹…: Open-Meteo's search found no 台北 and put 新竹 in 屏東). Rain likely
# (60% or more) in the morning: the island says so, once a day.
#
#   pwsh examples/widgets/weather.ps1 -City Shanghai
#   pwsh examples/widgets/weather.ps1 -City 台北 -Once
#   pwsh examples/widgets/weather.ps1 -City Berlin -Lang en
param(
  [string]$City = '',
  [int]$Port = 47213,
  [int]$Minutes = 10,
  [switch]$Once,
  # tw (Traditional Chinese, Taiwan's words), zh or en: as Windows' language or its formats are.
  [string]$Lang = $(
    $names = @((Get-UICulture).Name, (Get-Culture).Name)
    if ($names | Where-Object { $_ -match '^zh-(TW|HK|MO|Hant)' }) { 'tw' } elseif ($names | Where-Object { $_ -like 'zh*' }) { 'zh' } else { 'en' }
  )
)

if (-not $City.Trim()) { $City = $(if ($Lang -eq 'tw') { '台北' } else { 'Shanghai' }) }

# Taiwan's cities and counties, by their seats: no search needed.
$taiwan = @{
  '台北' = 25.0375, 121.5637; '新北' = 25.0120, 121.4659; '基隆' = 25.1276, 121.7392; '桃園' = 24.9936, 121.3010
  '新竹' = 24.8039, 120.9647; '苗栗' = 24.5602, 120.8214; '台中' = 24.1477, 120.6736; '彰化' = 24.0809, 120.5385
  '南投' = 23.9096, 120.6838; '雲林' = 23.7092, 120.5434; '嘉義' = 23.4801, 120.4491; '台南' = 22.9999, 120.2270
  '高雄' = 22.6273, 120.3014; '屏東' = 22.6762, 120.4929; '宜蘭' = 24.7570, 121.7530; '花蓮' = 23.9871, 121.6015
  '台東' = 22.7583, 121.1444; '澎湖' = 23.5711, 119.5793; '金門' = 24.4493, 118.3767; '馬祖' = 26.1597, 119.9497
}

# The sky by its WMO code: Traditional Chinese, Simplified, English.
$sky = @{
  0 = '晴', '晴', 'Clear'; 1 = '大致晴朗', '晴', 'Mostly clear'; 2 = '多雲', '多云', 'Partly cloudy'; 3 = '陰', '阴', 'Overcast'
  45 = '霧', '雾', 'Fog'; 48 = '霧', '雾', 'Fog'
  51 = '毛毛雨', '毛毛雨', 'Drizzle'; 53 = '毛毛雨', '毛毛雨', 'Drizzle'; 55 = '毛毛雨', '毛毛雨', 'Drizzle'; 56 = '凍雨', '冻雨', 'Freezing drizzle'; 57 = '凍雨', '冻雨', 'Freezing drizzle'
  61 = '小雨', '小雨', 'Light rain'; 63 = '中雨', '中雨', 'Rain'; 65 = '大雨', '大雨', 'Heavy rain'; 66 = '凍雨', '冻雨', 'Freezing rain'; 67 = '凍雨', '冻雨', 'Freezing rain'
  71 = '小雪', '小雪', 'Light snow'; 73 = '中雪', '中雪', 'Snow'; 75 = '大雪', '大雪', 'Heavy snow'; 77 = '雪粒', '雪粒', 'Snow grains'
  80 = '陣雨', '阵雨', 'Showers'; 81 = '陣雨', '阵雨', 'Showers'; 82 = '強陣雨', '强阵雨', 'Heavy showers'; 85 = '陣雪', '阵雪', 'Snow showers'; 86 = '陣雪', '阵雪', 'Snow showers'
  95 = '雷陣雨', '雷阵雨', 'Thunderstorm'; 96 = '雷陣雨伴冰雹', '雷阵雨伴冰雹', 'Thunderstorm, hail'; 99 = '雷陣雨伴冰雹', '雷阵雨伴冰雹', 'Thunderstorm, hail'
}
$pick = @{ tw = 0; zh = 1; en = 2 }[$Lang]
if ($null -eq $pick) { $pick = 2 }

# The day the island last said rain was coming (kept between runs).
$saidRain = Join-Path $env:TEMP 'wakuwaku-weather-rain.txt'

# Where the city is: Taiwan's from the table (臺 read as 台, 市/縣 left off),
# else found once, again next time if the network was down.
$place = $null
$key = ($City.Trim() -replace '臺', '台') -replace '[市縣县]$', ''
if ($taiwan.ContainsKey($key)) { $place = @{ latitude = $taiwan[$key][0]; longitude = $taiwan[$key][1] } }
while ($true) {
  try {
    if (-not $place) {
      $searchLang = $(if ($Lang -eq 'en') { 'en' } else { 'zh' })
      $found = Invoke-RestMethod "https://geocoding-api.open-meteo.com/v1/search?name=$([uri]::EscapeDataString($City))&count=1&language=$searchLang" -TimeoutSec 20
      $place = $found.results | Select-Object -First 1
      if (-not $place) { throw $(switch ($Lang) { 'tw' { "找不到這個城市：$City" } 'zh' { "找不到这个城市：$City" } default { "no city called $City" } }) }
    }
    $forecast = Invoke-RestMethod "https://api.open-meteo.com/v1/forecast?latitude=$($place.latitude)&longitude=$($place.longitude)&current=temperature_2m,weather_code&hourly=precipitation_probability&forecast_hours=12&timezone=auto" -TimeoutSec 20
    $now = $forecast.current
    $words = $sky[[int]$now.weather_code]
    $words = if ($words) { $words[$pick] } else { '' }
    $rain = [int](@($forecast.hourly.precipitation_probability | Where-Object { $null -ne $_ }) + 0 | Measure-Object -Maximum).Maximum
    $rainWord = $(if ($Lang -eq 'en') { "rain $rain%" } else { "降雨 $rain%" })
    $widget = @{
      id    = 'weather'
      icon  = 'weather'
      color = '#ffb340'
      label = (@($City, $words, $rainWord) | Where-Object { $_ }) -join ' · '
      value = "$([Math]::Round([double]$now.temperature_2m))°"
      # A little longer than the wait, so it stays between two updates.
      ttl   = $Minutes * 60 + 120
    }
    # Rain likely this morning (local time, as the forecast gives it): said once a day.
    $hour = [int]($now.time.Substring(11, 2))
    $today = $now.time.Substring(0, 10)
    $said = if (Test-Path $saidRain) { (Get-Content $saidRain -Raw).Trim() } else { '' }
    if ($rain -ge 60 -and $hour -ge 6 -and $hour -le 11 -and $said -ne $today) {
      $widget.nudge = $(switch ($Lang) { 'tw' { "今天可能下雨（$rain%），記得帶傘" } 'zh' { "今天可能下雨（$rain%），记得带伞" } default { "Rain likely today ($rain%): take an umbrella" } })
      Set-Content -Path $saidRain -Value $today
    }
    $json = $widget | ConvertTo-Json -Compress
    # As UTF-8 bytes: Windows PowerShell 5 would send a string in another encoding.
    Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($json)) | Out-Null
  } catch {
    Write-Warning $_
  }
  if ($Once) { break }
  Start-Sleep -Seconds ($Minutes * 60)
}
