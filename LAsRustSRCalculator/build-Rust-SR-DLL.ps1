cargo build --release
copy target\release\rust_sr_calculator.dll ..\src\Analysis\
cd ..
dotnet build src\LAsOsuBeatmapParser.csproj