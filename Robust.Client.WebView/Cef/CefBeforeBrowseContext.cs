namespace Robust.Client.WebView.Cef
{
    /// <summary>
    /// Before-browse context backed by plain strings from the native FFI layer.
    /// </summary>
    internal sealed class NativeBeforeBrowseContext : IBeforeBrowseContext
    {
        public string Url { get; }
        public string Method => "GET";
        public bool IsRedirect { get; }
        public bool UserGesture { get; }
        public bool IsCancelled { get; private set; }

        internal NativeBeforeBrowseContext(string url, bool isRedirect, bool userGesture)
        {
            Url = url;
            IsRedirect = isRedirect;
            UserGesture = userGesture;
        }

        public void DoCancel()
        {
            IsCancelled = true;
        }
    }
}
