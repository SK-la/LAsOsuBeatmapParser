# Rust SR Calculator

This is a Rust implementation of the Star Rating (SR) calculation algorithm for osu! mania beatmaps.

## Building

1. Install Rust: https://rustup.rs/
2. Build the library:
   ```bash
   cargo build --release
   ```

This will create `target/release/rust_sr_calculator.dll` (on Windows).

## Using in C#

To use the Rust SR calculator in your C# project:

1. Copy the `rust_sr_calculator.dll` to your C# project directory.

2. Add P/Invoke declarations:
   ```csharp
   using System.Runtime.InteropServices;

   [DllImport("rust_sr_calculator.dll", CallingConvention = CallingConvention.Cdecl)]
   private static extern IntPtr calculate_sr_from_json(IntPtr jsonPtr, int len);
   ```

3. Call the function:
   ```csharp
   private static double CalculateSRRust(Beatmap beatmap)
   {
       var beatmapData = new
       {
           difficulty_section = new
           {
               overall_difficulty = beatmap.DifficultySection.OverallDifficulty,
               circle_size = beatmap.DifficultySection.CircleSize
           },
           hit_objects = beatmap.HitObjects.Select(ho => new
           {
               position = new { x = ho.Position.X },
               start_time = ho.StartTime,
               end_time = ho.EndTime
           }).ToArray()
       };

       string json = System.Text.Json.JsonSerializer.Serialize(beatmapData);
       byte[] jsonBytes = System.Text.Encoding.UTF8.GetBytes(json);

       IntPtr resultPtr = calculate_sr_from_json(Marshal.StringToHGlobalAnsi(json), jsonBytes.Length);

       if (resultPtr == IntPtr.Zero)
       {
           throw new Exception("Rust SR calculation failed");
       }

       string resultJson = Marshal.PtrToStringAnsi(resultPtr);
       var result = System.Text.Json.JsonSerializer.Deserialize<Dictionary<string, double>>(resultJson);

       Marshal.FreeHGlobal(resultPtr);

       return result["sr"];
   }
   ```

## Running Tests

To run the comparison tests between C# and Rust implementations:

1. Update the test directory path in `SRCalculatorComparisonTests.cs`
2. Run the tests:
   ```bash
   dotnet test
   ```

## Performance

The Rust implementation uses parallel processing with Rayon for improved performance on multi-core systems.