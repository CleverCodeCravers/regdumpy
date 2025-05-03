# console application

- regdumpy is a rust console application
- it has a parameter that defines the output file / path, this is mandatory. 
- it has another parameter to select the registry root key we want to dump (e.g. HKEY_LOCAL_MACHINE)

- it shows a warning when not executed as administrator that not all registry values will be dumped due to permissions

- it will then walk recursivly through the complete registry
  - each "folder" it visits will be recorded
  - then all values of the folder will be recorded (key and value)
  - and afterwards subfolders will be visited, in case they are available

- before outputting a value it will check if the value is in a correct format (e.g. DWORD = 4 bytes, QWORD = 8 bytes, REG_SZ = string) sometimes values in the registry are not in the correct data format, which will make the program crash if we do not handle it
  - if you encounter invalid data write out the key and instead of the value write "invalid data" with your diagnosis why it is invalid

## general implementation
- create a subfolder "lib" with all necessary libraries that you need (do not write the code in just 1 file...)
- split the libraries in a way that makes sense
- the main program should only contain the absolutley necessary code

- if you can, write unit tests inside the code
- use the compiler and the unit tests to make sure the application can be built and works as expected.

