package net.kafji.belanda

import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.lifecycleScope
import io.grpc.ManagedChannelBuilder
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.kafji.belanda.HelloServiceGrpc.HelloServiceBlockingStub
import net.kafji.belanda.HelloServiceGrpc.HelloServiceStub
import net.kafji.belanda.ui.theme.BelandaRemoteTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        setContent {
            BelandaRemoteTheme {
                // A surface container using the 'background' color from the theme
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    Greeting("Android")
                }
            }
        }

        lifecycleScope.launch(Dispatchers.IO) {
            val host = ""
            val port = 0
            val channel = ManagedChannelBuilder.forAddress(host, port).usePlaintext().build();
            val stub = HelloServiceGrpc.newBlockingStub(channel)
            val message = HelloMessage.newBuilder().setName("Budi").build()
            val reply = stub.hello(message)
            Log.d("kek", reply.greeting)
        }
    }
}

@Composable
fun Greeting(name: String, modifier: Modifier = Modifier) {
    Text(
        text = "Hello $name!",
        modifier = modifier
    )
}

@Preview(showBackground = true)
@Composable
fun GreetingPreview() {
    BelandaRemoteTheme {
        Greeting("Android")
    }
}
