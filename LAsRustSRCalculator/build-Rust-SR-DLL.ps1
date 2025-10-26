cd LAsRustSRCalculator
cargo build --release
copy target\release\rust_sr_calculator.dll ..\src\Analysis\
cd ..
dotnet build src\LAsOsuBeatmapParser.csproj
dotnet test tests\LAsOsuBeatmapParser.Tests.csproj --filter RustSRCalculatorTests