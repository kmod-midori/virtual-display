set shell := ["powershell.exe", "-c"]

fbs:
    Remove-Item -Force -Recurse protocol
    flatc -o protocol\rust   --rust   protocol.fbs
    flatc -o protocol\kotlin --kotlin protocol.fbs
    flatc -o protocol\ts     --ts     protocol.fbs
