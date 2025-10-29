# publish-nuget.ps1
# 直接右键运行即可，无需传参。需要发布正式版或 beta 版时，只需修改 $Version。

$Version = "3.0.0-beta"   # 需要发布正式版时改为如 "2.0.0"，发布 beta 时改为 "3.0.0-beta"
$ApiKey = $env:NUGET_API_KEY  # 从环境变量获取 API Key

$csproj = "e:\BASE CODE\GitHub\LAsOsuBeatmapParser\src\LAsOsuBeatmapParser.csproj"

Write-Host "切换版本号为 $Version ..."
(Get-Content $csproj) -replace '<Version>.*?</Version>', "<Version>$Version</Version>" | Set-Content $csproj

Write-Host "开始打包..."
dotnet pack $csproj -c Release

$nupkg = Get-ChildItem -Path "e:\BASE CODE\GitHub\LAsOsuBeatmapParser\src\bin\Release" -Filter "*.nupkg" | Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($nupkg) {
    Write-Host "推送到 NuGet.org ..."
    dotnet nuget push $nupkg.FullName --api-key $ApiKey --source https://api.nuget.org/v3/index.json
    Write-Host "发布完成！"
} else {
    Write-Host "未找到 nupkg 文件，打包失败。"
}

# 可选：恢复为 beta 版本（自动用 $Version 变量，发布后如需回到 beta 状态，先手动改 $Version 再取消注释）
# (Get-Content $csproj) -replace '<Version>.*?</Version>', "<Version>$Version</Version>" | Set-Content $csproj
