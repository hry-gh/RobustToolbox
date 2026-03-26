using System;
using System.IO;
using System.Net;

namespace Robust.Client.WebView.Cef
{
    /// <summary>
    /// Holds response data to be sent back through the native FFI.
    /// </summary>
    internal sealed class NativeResponseData
    {
        public int StatusCode { get; }
        public string MimeType { get; }
        public byte[] Data { get; }

        public NativeResponseData(int statusCode, string mimeType, byte[] data)
        {
            StatusCode = statusCode;
            MimeType = mimeType;
            Data = data;
        }
    }

    /// <summary>
    /// Request handler context backed by plain strings from the native FFI layer.
    /// </summary>
    internal sealed class NativeRequestHandlerContext : IRequestHandlerContext
    {
        public bool IsNavigation => false;
        public bool IsDownload => false;
        public string RequestInitiator => "";
        public string Url { get; }
        public string Method { get; }
        public bool IsHandled { get; private set; }
        public bool IsCancelled { get; private set; }

        internal NativeResponseData? ResponseData { get; private set; }

        internal NativeRequestHandlerContext(string url, string method)
        {
            Url = url;
            Method = method;
        }

        public void DoCancel()
        {
            if (IsHandled)
                throw new InvalidOperationException("Request has already been handled");

            IsHandled = true;
            IsCancelled = true;
        }

        public void DoRespondStream(Stream stream, string contentType, HttpStatusCode code = HttpStatusCode.OK)
        {
            using var ms = new MemoryStream();
            stream.CopyTo(ms);
            var data = ms.ToArray();

            ResponseData = new NativeResponseData((int)code, contentType, data);
            IsHandled = true;
        }
    }
}
