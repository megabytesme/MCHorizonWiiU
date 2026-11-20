using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text.Json;

try
{
    switch (args)
    {
        case ["self-test", var catalog]:
        {
            var bytes = ReadCatalog(catalog, requireSigned: false);
            using var key = ECDsa.Create(ECCurve.NamedCurves.nistP256);
            var signature = Sign(bytes, key);
            var publicKey = key.ExportSubjectPublicKeyInfo();
            if (!Verify(bytes, signature, publicKey))
                throw new InvalidDataException("Ephemeral signature failed.");
            bytes[0] ^= 1;
            if (Verify(bytes, signature, publicKey))
                throw new InvalidDataException("Modified catalog was accepted.");
            using var other = ECDsa.Create(ECCurve.NamedCurves.nistP256);
            bytes[0] ^= 1;
            if (Verify(bytes, signature, other.ExportSubjectPublicKeyInfo()))
                throw new InvalidDataException("An untrusted key was accepted.");
            Console.WriteLine("Ephemeral signing, tamper detection and trust checks passed.");
            break;
        }
        case ["sign", var catalog, var destination, var thumbprint]:
        {
            var bytes = ReadCatalog(catalog, requireSigned: true);
            using var key = OpenSigningKey(thumbprint);
            WriteNew(destination, Sign(bytes, key));
            break;
        }
        case ["export-public", var destination, var thumbprint]:
        {
            using var key = OpenSigningKey(thumbprint);
            WriteNew(destination, key.ExportSubjectPublicKeyInfo());
            break;
        }
        case ["verify", var catalog, var signature, var publicKey]:
        {
            var bytes = ReadCatalog(catalog, requireSigned: true);
            if (!Verify(bytes, File.ReadAllBytes(signature), File.ReadAllBytes(publicKey)))
                throw new InvalidDataException("Catalog signature or pinned public key is invalid.");
            Console.WriteLine("Detached catalog signature verified.");
            break;
        }
        default:
            throw new ArgumentException("Usage: self-test <catalog> | sign <catalog> <new.sig> <thumbprint> | export-public <new.der> <thumbprint> | verify <catalog> <sig> <trusted.der>");
    }
}
catch (Exception error)
{
    Console.Error.WriteLine(error.Message);
    return 1;
}
return 0;

static byte[] ReadCatalog(string path, bool requireSigned)
{
    using var input = File.OpenRead(path);
    if (input.Length > 1024 * 1024)
        throw new InvalidDataException("Catalog exceeds the size limit.");
    var data = new byte[input.Length];
    input.ReadExactly(data);
    using var document = JsonDocument.Parse(data);
    var root = document.RootElement;
    RequireUniqueProperties(root);
    if (root.GetProperty("schema").GetInt32() != 2 ||
        (requireSigned && root.GetProperty("signing").GetString() != "p256-sha256-p1363"))
        throw new InvalidDataException("Expected a schema 2 catalog declared ready for signing.");
    return data;
}

static void RequireUniqueProperties(JsonElement value)
{
    if (value.ValueKind == JsonValueKind.Object)
    {
        var names = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in value.EnumerateObject())
        {
            if (!names.Add(property.Name))
                throw new InvalidDataException($"Duplicate catalog field: {property.Name}");
            RequireUniqueProperties(property.Value);
        }
    }
    else if (value.ValueKind == JsonValueKind.Array)
    {
        foreach (var item in value.EnumerateArray())
            RequireUniqueProperties(item);
    }
}

static byte[] Sign(byte[] data, ECDsa key) =>
    key.SignData(data, HashAlgorithmName.SHA256, DSASignatureFormat.IeeeP1363FixedFieldConcatenation);

static bool Verify(byte[] data, byte[] signature, byte[] publicKey)
{
    if (signature.Length != 64 || publicKey.Length > 256)
        return false;
    using var key = ECDsa.Create();
    key.ImportSubjectPublicKeyInfo(publicKey, out var consumed);
    return consumed == publicKey.Length && IsP256(key) &&
        key.VerifyData(data, signature, HashAlgorithmName.SHA256, DSASignatureFormat.IeeeP1363FixedFieldConcatenation);
}

static bool IsP256(ECDsa key) =>
    key.KeySize == 256 && key.ExportParameters(false).Curve.Oid.Value == "1.2.840.10045.3.1.7";

static ECDsa OpenSigningKey(string thumbprint)
{
    if (!OperatingSystem.IsWindows() || thumbprint.Length != 40 || !thumbprint.All(char.IsAsciiHexDigit))
        throw new ArgumentException("Use Windows and provide the signing certificate's 40-digit thumbprint.");
    using var store = new X509Store(StoreName.My, StoreLocation.CurrentUser);
    store.Open(OpenFlags.ReadOnly | OpenFlags.OpenExistingOnly);
    var matches = store.Certificates.Find(X509FindType.FindByThumbprint, thumbprint, validOnly: false);
    if (matches.Count != 1 || !matches[0].HasPrivateKey ||
        matches[0].NotBefore > DateTime.Now || matches[0].NotAfter <= DateTime.Now)
        throw new InvalidOperationException("A current certificate with a private key is required in CurrentUser\\My.");
    using var certificate = matches[0];
    var key = certificate.GetECDsaPrivateKey();
    if (key is not null && IsP256(key))
        return key;
    key?.Dispose();
    throw new InvalidOperationException("The certificate must use ECDSA P-256.");
}

static void WriteNew(string path, byte[] data)
{
    using var output = new FileStream(path, FileMode.CreateNew, FileAccess.Write, FileShare.None);
    output.Write(data);
}
