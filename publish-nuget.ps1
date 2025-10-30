# publish-nuget.ps1
# Run directly by right-clicking, no parameters needed. Modify $Version for release or beta.

$Version = "1.0.3-beta"   # For release, change to e.g. "2.0.0", for beta "3.0.0-beta"
$ApiKey = $env:NUGET_API_KEY

if (-not $ApiKey) {
    Write-Host "Error: Environment variable NUGET_API_KEY is not set. Please set the API Key first."
    exit 1
}

$csproj = "e:\BASE CODE\GitHub\LAsOsuBeatmapParser\src\LAsOsuBeatmapParser.csproj"

Write-Host "Switching version to $Version ..."
$pattern = '<Version>.*?</Version>'
$replacement = "<Version>$Version</Version>"
(Get-Content $csproj) -replace $pattern, $replacement | Set-Content $csproj

Write-Host "Starting packaging..."
dotnet pack $csproj -c Release

$nupkg = Get-ChildItem -Path "e:\BASE CODE\GitHub\LAsOsuBeatmapParser\src\bin\Release" -Filter "*.nupkg" | Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($nupkg) {
    Write-Host "Pushing to NuGet.org ..."
    dotnet nuget push $nupkg.FullName --api-key $ApiKey --source https://api.nuget.org/v3/index.json
    Write-Host "Publishing completed!"
} else {
    Write-Host "No nupkg file found, packaging failed."
}

# Optional: Restore to beta version (uses $Version variable, uncomment after publishing if needed to return to beta)
# (Get-Content $csproj) -replace $pattern, $replacement | Set-Content $csproj