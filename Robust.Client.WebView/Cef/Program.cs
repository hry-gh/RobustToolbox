using System;
using System.Diagnostics;
using System.Globalization;
using System.Threading;

namespace Robust.Client.WebView.Cef
{
    internal static class Program
    {
        // The subprocess entry point is now handled by the Rust cef-helper binary.
        // This C# entry point is kept for compatibility but should not be used for CEF subprocesses.
        public static int Main(string[] args)
        {
            StartWatchThread();

            // The actual subprocess is the Rust cef-helper. This C# program should not be invoked
            // as a CEF subprocess anymore. If it is, just exit.
            System.Console.Error.WriteLine("CEF subprocess should use the Rust cef-helper binary, not this C# program.");
            return 1;
        }

        private static void StartWatchThread()
        {
            //
            // CEF has this nasty habit of not shutting down all its processes if the parent crashes.
            // Great!
            //
            // We use a separate thread in each CEF child process to watch the main PID.
            // If it exits, we kill ourselves after a couple seconds.
            //

            if (Environment.GetEnvironmentVariable("ROBUST_CEF_BROWSER_PROCESS_ID") is not { } parentIdString)
                return;

            if (Environment.GetEnvironmentVariable("ROBUST_CEF_BROWSER_PROCESS_MODULE") is not { } parentModuleString)
                return;

            if (!int.TryParse(parentIdString, CultureInfo.InvariantCulture, out var parentId))
                return;

            var process = Process.GetProcessById(parentId);
            if ((process.MainModule?.FileName ?? "") != parentModuleString)
            {
                process.Dispose();
                return;
            }

            new Thread(() => WatchThread(process)) { Name = "CEF Watch Thread", IsBackground = true }
                .Start();
        }

        private static void WatchThread(Process p)
        {
            p.WaitForExit();

            Thread.Sleep(3000);

            Environment.Exit(1);
        }
    }
}
