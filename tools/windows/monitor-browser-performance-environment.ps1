param(
    [Parameter(Mandatory = $true)]
    [int]$RootProcessId,
    [ValidateRange(250, 10000)]
    [int]$SampleIntervalMilliseconds = 1000
)

$ErrorActionPreference = "Stop"

$nativeSource = @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;

public sealed class VineCounterValues
{
    public double? TotalCpuPercent { get; set; }
    public double? PhysicalDiskBytesPerSecond { get; set; }
    public double? PhysicalDiskBusyPercent { get; set; }
}

public sealed class VinePdhCounters : IDisposable
{
    private const UInt32 PdhFormatDouble = 0x00000200;
    private IntPtr query;
    private IntPtr cpu;
    private IntPtr diskBytes;
    private IntPtr diskBusy;

    [StructLayout(LayoutKind.Explicit)]
    private struct PdhFormattedCounterValue
    {
        [FieldOffset(0)] public UInt32 Status;
        [FieldOffset(8)] public double DoubleValue;
    }

    [DllImport("pdh.dll", CharSet = CharSet.Unicode)]
    private static extern UInt32 PdhOpenQueryW(string dataSource, UIntPtr userData, out IntPtr query);

    [DllImport("pdh.dll", CharSet = CharSet.Unicode, EntryPoint = "PdhAddEnglishCounterW")]
    private static extern UInt32 PdhAddEnglishCounter(
        IntPtr query,
        string counterPath,
        UIntPtr userData,
        out IntPtr counter);

    [DllImport("pdh.dll")]
    private static extern UInt32 PdhCollectQueryData(IntPtr query);

    [DllImport("pdh.dll")]
    private static extern UInt32 PdhGetFormattedCounterValue(
        IntPtr counter,
        UInt32 format,
        out UInt32 type,
        out PdhFormattedCounterValue value);

    [DllImport("pdh.dll")]
    private static extern UInt32 PdhCloseQuery(IntPtr query);

    public VinePdhCounters()
    {
        Check(PdhOpenQueryW(null, UIntPtr.Zero, out query), "PdhOpenQuery");
        Check(PdhAddEnglishCounter(query, @"\Processor(_Total)\% Processor Time", UIntPtr.Zero, out cpu), "CPU counter");
        Check(PdhAddEnglishCounter(query, @"\PhysicalDisk(_Total)\Disk Bytes/sec", UIntPtr.Zero, out diskBytes), "disk byte counter");
        Check(PdhAddEnglishCounter(query, @"\PhysicalDisk(_Total)\% Disk Time", UIntPtr.Zero, out diskBusy), "disk busy counter");
    }

    private static void Check(UInt32 status, string operation)
    {
        if (status != 0)
        {
            throw new InvalidOperationException(operation + " failed with PDH status 0x" + status.ToString("x8"));
        }
    }

    public void Prime()
    {
        Check(PdhCollectQueryData(query), "initial PDH collection");
    }

    private static double? Read(IntPtr counter)
    {
        UInt32 type;
        PdhFormattedCounterValue value;
        UInt32 status = PdhGetFormattedCounterValue(counter, PdhFormatDouble, out type, out value);
        if (status != 0 || value.Status != 0 || Double.IsNaN(value.DoubleValue) || Double.IsInfinity(value.DoubleValue))
        {
            return null;
        }
        return value.DoubleValue;
    }

    public VineCounterValues Sample()
    {
        Check(PdhCollectQueryData(query), "PDH collection");
        return new VineCounterValues
        {
            TotalCpuPercent = Read(cpu),
            PhysicalDiskBytesPerSecond = Read(diskBytes),
            PhysicalDiskBusyPercent = Read(diskBusy),
        };
    }

    public void Dispose()
    {
        if (query != IntPtr.Zero)
        {
            PdhCloseQuery(query);
            query = IntPtr.Zero;
        }
    }
}

public sealed class VineProcessEntry
{
    public Int32 ProcessId { get; set; }
    public Int32 ParentProcessId { get; set; }
    public string Name { get; set; }
}

public sealed class VineProcessObservation
{
    public Int32 ProcessId { get; set; }
    public string Name { get; set; }
    public string ExecutablePath { get; set; }
    public UInt64 CreationTime100ns { get; set; }
    public UInt64 CpuTime100ns { get; set; }
    public UInt64 IoTransferBytes { get; set; }
    public UInt32 PriorityClass { get; set; }
    public bool PriorityBoostDisabled { get; set; }
    public UInt32 PowerThrottlingControlMask { get; set; }
    public UInt32 PowerThrottlingStateMask { get; set; }
}

public static class VineProcessNative
{
    private const UInt32 SnapshotProcesses = 0x00000002;
    private const UInt32 QueryLimitedInformation = 0x00001000;
    private static readonly IntPtr InvalidHandle = new IntPtr(-1);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct ProcessEntry32
    {
        public UInt32 Size;
        public UInt32 Usage;
        public UInt32 ProcessId;
        public IntPtr DefaultHeapId;
        public UInt32 ModuleId;
        public UInt32 Threads;
        public UInt32 ParentProcessId;
        public Int32 BasePriority;
        public UInt32 Flags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string ExeFile;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct FileTime
    {
        public UInt32 Low;
        public UInt32 High;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct IoCounters
    {
        public UInt64 ReadOperationCount;
        public UInt64 WriteOperationCount;
        public UInt64 OtherOperationCount;
        public UInt64 ReadTransferCount;
        public UInt64 WriteTransferCount;
        public UInt64 OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct ProcessPowerThrottlingState
    {
        public UInt32 Version;
        public UInt32 ControlMask;
        public UInt32 StateMask;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr CreateToolhelp32Snapshot(UInt32 flags, UInt32 processId);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool Process32FirstW(IntPtr snapshot, ref ProcessEntry32 entry);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool Process32NextW(IntPtr snapshot, ref ProcessEntry32 entry);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr OpenProcess(UInt32 access, bool inheritHandle, UInt32 processId);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetProcessTimes(
        IntPtr process,
        out FileTime creation,
        out FileTime exit,
        out FileTime kernel,
        out FileTime user);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetProcessIoCounters(IntPtr process, out IoCounters counters);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern UInt32 GetPriorityClass(IntPtr process);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetProcessPriorityBoost(IntPtr process, out bool disabled);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetProcessInformation(
        IntPtr process,
        Int32 processInformationClass,
        ref ProcessPowerThrottlingState information,
        UInt32 informationSize);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool QueryFullProcessImageNameW(
        IntPtr process,
        UInt32 flags,
        StringBuilder name,
        ref UInt32 size);

    private static UInt64 Combine(FileTime value)
    {
        return ((UInt64)value.High << 32) | value.Low;
    }

    public static VineProcessEntry[] EnumerateProcesses()
    {
        IntPtr snapshot = CreateToolhelp32Snapshot(SnapshotProcesses, 0);
        if (snapshot == InvalidHandle)
        {
            throw new Win32Exception(Marshal.GetLastWin32Error(), "CreateToolhelp32Snapshot failed");
        }
        try
        {
            var entries = new List<VineProcessEntry>();
            var native = new ProcessEntry32();
            native.Size = (UInt32)Marshal.SizeOf(typeof(ProcessEntry32));
            if (!Process32FirstW(snapshot, ref native))
            {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "Process32First failed");
            }
            do
            {
                entries.Add(new VineProcessEntry
                {
                    ProcessId = unchecked((Int32)native.ProcessId),
                    ParentProcessId = unchecked((Int32)native.ParentProcessId),
                    Name = native.ExeFile,
                });
                native.Size = (UInt32)Marshal.SizeOf(typeof(ProcessEntry32));
            }
            while (Process32NextW(snapshot, ref native));
            return entries.ToArray();
        }
        finally
        {
            CloseHandle(snapshot);
        }
    }

    public static VineProcessObservation Observe(Int32 processId, string name)
    {
        IntPtr process = OpenProcess(QueryLimitedInformation, false, unchecked((UInt32)processId));
        if (process == IntPtr.Zero)
        {
            return null;
        }
        try
        {
            FileTime creation;
            FileTime exit;
            FileTime kernel;
            FileTime user;
            IoCounters io;
            bool boostDisabled;
            if (!GetProcessTimes(process, out creation, out exit, out kernel, out user) ||
                !GetProcessIoCounters(process, out io) ||
                !GetProcessPriorityBoost(process, out boostDisabled))
            {
                return null;
            }
            UInt32 priorityClass = GetPriorityClass(process);
            if (priorityClass == 0)
            {
                return null;
            }
            var power = new ProcessPowerThrottlingState
            {
                Version = 1,
                ControlMask = 0,
                StateMask = 0,
            };
            if (!GetProcessInformation(
                process,
                4,
                ref power,
                (UInt32)Marshal.SizeOf(typeof(ProcessPowerThrottlingState))))
            {
                return null;
            }
            var path = new StringBuilder(32768);
            UInt32 pathLength = (UInt32)path.Capacity;
            string executablePath = QueryFullProcessImageNameW(process, 0, path, ref pathLength)
                ? path.ToString()
                : null;
            return new VineProcessObservation
            {
                ProcessId = processId,
                Name = name,
                ExecutablePath = executablePath,
                CreationTime100ns = Combine(creation),
                CpuTime100ns = Combine(kernel) + Combine(user),
                IoTransferBytes = io.ReadTransferCount + io.WriteTransferCount,
                PriorityClass = priorityClass,
                PriorityBoostDisabled = boostDisabled,
                PowerThrottlingControlMask = power.ControlMask,
                PowerThrottlingStateMask = power.StateMask,
            };
        }
        finally
        {
            CloseHandle(process);
        }
    }
}
'@

Add-Type -TypeDefinition $nativeSource -Language CSharp

function Write-Ndjson {
    param([Parameter(Mandatory = $true)]$Value)
    [Console]::Out.WriteLine(($Value | ConvertTo-Json -Compress -Depth 8))
    [Console]::Out.Flush()
}

$logicalCpus = [Environment]::ProcessorCount
$stopwatch = [Diagnostics.Stopwatch]::StartNew()
$previousElapsedMilliseconds = 0.0
$previousProcesses = @{}
$sequence = 0
$counters = [VinePdhCounters]::new()

try {
    $counters.Prime()
    Write-Ndjson ([ordered]@{
        kind = "ready"
        schemaVersion = 1
        observedAtUtc = [DateTime]::UtcNow.ToString("o")
        rootProcessId = $RootProcessId
        logicalCpus = $logicalCpus
        sampleIntervalMilliseconds = $SampleIntervalMilliseconds
        counters = @(
            "total-cpu-percent",
            "physical-disk-bytes-per-second",
            "physical-disk-busy-percent",
            "chrome-process-cpu-io-priority-power-throttling",
            "updater-process-lifecycle"
        )
    })

    while ($true) {
        Start-Sleep -Milliseconds $SampleIntervalMilliseconds
        $elapsedMilliseconds = $stopwatch.Elapsed.TotalMilliseconds
        $elapsedSeconds = [Math]::Max(
            0.001,
            ($elapsedMilliseconds - $previousElapsedMilliseconds) / 1000.0
        )
        $counterValues = $counters.Sample()
        $processEntries = [VineProcessNative]::EnumerateProcesses()
        $entryById = @{}
        $descendantIds = [Collections.Generic.HashSet[int]]::new()
        [void]$descendantIds.Add($RootProcessId)
        foreach ($entry in $processEntries) {
            $entryById[$entry.ProcessId] = $entry
        }
        $changed = $true
        while ($changed) {
            $changed = $false
            foreach ($entry in $processEntries) {
                if (-not $descendantIds.Contains($entry.ProcessId) -and
                    $descendantIds.Contains($entry.ParentProcessId)) {
                    [void]$descendantIds.Add($entry.ProcessId)
                    $changed = $true
                }
            }
        }

        $browserProcesses = @()
        $currentProcesses = @{}
        [double]$browserCpuPercent = 0.0
        [double]$browserIoBytesPerSecond = 0.0
        $browserDeltaAvailable = $false
        foreach ($processId in $descendantIds) {
            if (-not $entryById.ContainsKey($processId)) {
                continue
            }
            $entry = $entryById[$processId]
            $observation = [VineProcessNative]::Observe($processId, $entry.Name)
            if ($null -eq $observation) {
                continue
            }
            $identity = "{0}:{1}" -f $observation.ProcessId, $observation.CreationTime100ns
            $currentProcesses[$identity] = $observation
            if ($previousProcesses.ContainsKey($identity)) {
                $previous = $previousProcesses[$identity]
                if ($observation.CpuTime100ns -ge $previous.CpuTime100ns -and
                    $observation.IoTransferBytes -ge $previous.IoTransferBytes) {
                    $cpuSeconds = [double]($observation.CpuTime100ns - $previous.CpuTime100ns) / 10000000.0
                    $browserCpuPercent += ($cpuSeconds / $elapsedSeconds / $logicalCpus) * 100.0
                    $browserIoBytesPerSecond += [double]($observation.IoTransferBytes - $previous.IoTransferBytes) / $elapsedSeconds
                    $browserDeltaAvailable = $true
                }
            }
            $browserProcesses += [ordered]@{
                identity = $identity
                processId = $observation.ProcessId
                name = $observation.Name
                executablePath = $observation.ExecutablePath
                priorityClass = $observation.PriorityClass
                priorityBoostDisabled = $observation.PriorityBoostDisabled
                powerThrottlingControlMask = $observation.PowerThrottlingControlMask
                powerThrottlingStateMask = $observation.PowerThrottlingStateMask
            }
        }

        $updateCandidates = @()
        foreach ($entry in $processEntries) {
            if ($entry.Name -notmatch "(?i)update") {
                continue
            }
            $observation = [VineProcessNative]::Observe($entry.ProcessId, $entry.Name)
            if ($null -ne $observation) {
                $updateCandidates += [ordered]@{
                    processId = $observation.ProcessId
                    name = $observation.Name
                    executablePath = $observation.ExecutablePath
                    identity = "{0}:{1}" -f $observation.ProcessId, $observation.CreationTime100ns
                }
            }
        }

        $totalCpuPercent = $counterValues.TotalCpuPercent
        $externalCpuPercent = $null
        if ($null -ne $totalCpuPercent -and $browserDeltaAvailable) {
            $externalCpuPercent = [Math]::Max(0.0, [double]$totalCpuPercent - $browserCpuPercent)
        }
        Write-Ndjson ([ordered]@{
            kind = "sample"
            schemaVersion = 1
            sequence = $sequence
            observedAtUtc = [DateTime]::UtcNow.ToString("o")
            elapsedMilliseconds = $elapsedMilliseconds
            rootProcessPresent = $descendantIds.Contains($RootProcessId) -and $entryById.ContainsKey($RootProcessId)
            totalCpuPercent = $totalCpuPercent
            browserCpuPercent = $(if ($browserDeltaAvailable) { $browserCpuPercent } else { $null })
            externalCpuPercent = $externalCpuPercent
            physicalDiskBytesPerSecond = $counterValues.PhysicalDiskBytesPerSecond
            physicalDiskBusyPercent = $counterValues.PhysicalDiskBusyPercent
            browserIoBytesPerSecond = $(if ($browserDeltaAvailable) { $browserIoBytesPerSecond } else { $null })
            browserProcesses = $browserProcesses
            updateCandidates = $updateCandidates
        })
        $previousProcesses = $currentProcesses
        $previousElapsedMilliseconds = $elapsedMilliseconds
        $sequence += 1
    }
}
catch {
    Write-Ndjson ([ordered]@{
        kind = "error"
        schemaVersion = 1
        observedAtUtc = [DateTime]::UtcNow.ToString("o")
        message = $_.Exception.Message
        category = $_.CategoryInfo.Category.ToString()
    })
    exit 1
}
finally {
    $counters.Dispose()
}
