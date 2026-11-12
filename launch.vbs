Option Explicit
Dim shell, fso, repoDir, pythonwPath
Set shell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")
repoDir = fso.GetParentFolderName(WScript.ScriptFullName)
shell.CurrentDirectory = repoDir

pythonwPath = shell.ExpandEnvironmentStrings("%LOCALAPPDATA%\Programs\Python\Python312\pythonw.exe")
If Not fso.FileExists(pythonwPath) Then
    pythonwPath = "pythonw.exe"
End If

shell.Run """" & pythonwPath & """ -m notch.main", 0, False
