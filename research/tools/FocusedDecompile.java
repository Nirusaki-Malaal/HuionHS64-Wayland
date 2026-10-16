// Local analysis helper; generated pseudocode stays outside the repository.
// @category Huion
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import java.io.*;

public class FocusedDecompile extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected output path and function-name regex");
        }
        DecompInterface decompiler = new DecompInterface();
        try (PrintWriter out = new PrintWriter(new FileWriter(args[0]))) {
            decompiler.openProgram(currentProgram);
            FunctionIterator functions = currentProgram.getFunctionManager().getFunctions(true);
            while (functions.hasNext() && !monitor.isCancelled()) {
                Function function = functions.next();
                if (function.getName().matches(args[1])) {
                    out.println("// Ghidra address: " + function.getEntryPoint());
                    out.println("// Name: " + function.getName(true));
                    DecompileResults result = decompiler.decompileFunction(function, 45, monitor);
                    if (result.decompileCompleted()) {
                        out.println(result.getDecompiledFunction().getC());
                    } else {
                        out.println("// Failed: " + result.getErrorMessage());
                    }
                    out.flush();
                }
            }
        } finally {
            decompiler.dispose();
        }
    }
}
