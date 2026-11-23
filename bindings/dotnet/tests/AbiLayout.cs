using System.Globalization;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

internal sealed class AbiLayout(string path, string prefix)
{
    private readonly Dictionary<(string Type, string Field), int> expected = Read(path);

    private static Dictionary<(string Type, string Field), int> Read(string path)
    {
        var fields = new Dictionary<(string Type, string Field), int>();
        foreach (string line in File.ReadLines(path))
        {
            string[] parts = line.Split(' ');
            if (parts is not [var type, var field, var size])
                throw new InvalidDataException("Expected a C type, field and layout value.");
            fields.Add((type, field), int.Parse(size, NumberStyles.None, CultureInfo.InvariantCulture));
        }
        return fields;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct AlignmentProbe<T> where T : unmanaged
    {
        internal byte Prefix;
        internal T Value;
    }

    internal void Check<T>(string nativeType, params (string Native, string Managed)[] fields)
        where T : unmanaged
    {
        string name = prefix + nativeType;
        AlignmentProbe<T> probe = default;
        nint alignment = Unsafe.ByteOffset(ref probe.Prefix, ref Unsafe.As<T, byte>(ref probe.Value));
        Match(name, "size", Unsafe.SizeOf<T>());
        Match(name, "alignment", checked((int)alignment));
        foreach (var (native, managed) in fields)
            Match(name, native, Marshal.OffsetOf<T>(managed).ToInt32());
    }

    private void Match(string type, string field, int managed)
    {
        if (!expected.Remove((type, field), out int native))
            throw new InvalidDataException($"The C fixture omitted {type}.{field}.");
        if (native != managed)
            throw new InvalidOperationException($"{type}.{field} has C layout {native} and .NET layout {managed}.");
    }

    internal void CheckComplete()
    {
        if (expected.Count != 0)
            throw new InvalidDataException("The managed fixture did not check every reported C layout.");
        Console.WriteLine($"C and .NET {prefix} layouts agree.");
    }
}
